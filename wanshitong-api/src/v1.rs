//! `/api/v1`

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// `GET /api/v1/health`: that the service is up, for a deployment's health
/// check, and which version it runs.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Health {
    pub status: HealthStatus,
    /// The service's version, e.g. `0.1.0`, for a bug report.
    pub version: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Ok,
}

/// The body of every error response.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: Error,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Error {
    pub code: ErrorCode,
    /// For people, not for matching on.
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The request is malformed, e.g. an invalid cursor.
    BadRequest,
    /// Nothing has the requested identifier.
    NotFound,
    /// The request conflicts with WST's state, e.g. a deleted data source.
    Conflict,
    /// There is no canonical catalog yet, because nothing has been ingested.
    NoCatalog,
    /// No knowledge base is configured, e.g. no TMDB token.
    NoKnowledgeBase,
    /// A knowledge base failed to answer.
    KnowledgeBaseFailed,
    Internal,
    /// A code this client doesn't know, from a newer server.
    #[serde(other)]
    Unknown,
}

/// The interpretation that a response was read from.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Interpretation {
    pub uuid: Uuid,
    pub name: String,
    pub canonical: bool,
    pub state: InterpretationState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationState {
    /// Its catalog is being derived.
    Running,
    /// Stopped without finishing, e.g. because the server restarted.
    Interrupted,
    Failed,
    Succeeded,
}

/// `GET /api/v1/unidentified`
#[derive(Clone, Debug, Default, Deserialize, Serialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UnidentifiedQuery {
    /// The canonical interpretation's by default. A cursor keeps reading the
    /// interpretation it came from.
    pub interpretation: Option<Uuid>,
    /// Only files whose path contains this, ignoring case.
    pub name: Option<String>,
    /// From a previous page's `next_cursor`.
    pub cursor: Option<String>,
    /// Files per page. The server caps it.
    pub limit: Option<usize>,
}

/// The video files that no identified work includes: what's left to identify.
/// Sorted by data source name, then path, so each top directory's files are
/// together.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct UnidentifiedVideos {
    pub interpretation: Interpretation,
    pub files: Vec<FileSummary>,
    /// `None` on the last page.
    pub next_cursor: Option<String>,
}

/// A file, named by data source and path, which stay valid across
/// interpretations.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct FileSummary {
    pub data_source: DataSource,
    /// Relative to the data source's root.
    pub path: String,
    pub size_bytes: Option<i64>,
    /// The disc it stands for, if it's a disc's index file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disc: Option<Disc>,
}

/// A disc structure, e.g. a ripped Blu-ray, which its index file stands for:
/// assertions name it by that file, never its other files. See *Disc* in
/// `spec/domain-model.md`.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Disc {
    pub kind: DiscKind,
    /// How many files it has, its index file among them.
    pub file_count: u64,
    /// Its longest title's, if its files' durations were measured.
    pub duration_seconds: Option<f64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiscKind {
    BluRay,
    Dvd,
}

/// `GET /api/v1/files/evidence`
#[derive(Clone, Debug, Deserialize, Serialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FileQuery {
    /// The canonical interpretation by default.
    pub interpretation: Option<Uuid>,
    pub data_source: Uuid,
    /// Relative to the data source's root.
    pub path: String,
    /// Whether to show the whole top directory, listing its files, rather than
    /// summarise what doesn't lead to the file.
    #[serde(default)]
    pub directory_files: bool,
}

/// Everything WST knows about a file, e.g. an unidentified video, to identify
/// it by.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct FileEvidence {
    /// The interpretation that saw the file.
    pub interpretation: Interpretation,
    pub file: File,
    /// The top directory the file is in, unless it's at its data source's root.
    pub top_directory: Option<TopDirectory>,
    /// The assertions that the interpretation included about the file, newest
    /// first.
    pub assertions: Vec<Assertion>,
}

/// `GET /api/v1/works/{uuid}`
#[derive(Clone, Debug, Default, Deserialize, Serialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct WorkQuery {
    /// Whether to list the files in each directory, rather than only
    /// count and summarise them.
    #[serde(default)]
    pub directory_files: bool,
}

/// `GET /api/v1/works/{uuid}`: everything WST knows about how it derived a
/// work, to identify it by.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct WorkEvidence {
    /// The interpretation whose catalog the work is in.
    pub interpretation: Interpretation,
    pub work: Work,
    /// The work's files. A series' are in episode order, then its extras. The
    /// files of a movie's discs, e.g. a Blu-ray's streams, are left out: its
    /// top directories summarise them.
    pub files: Vec<File>,
    /// The top directories the work's files are in: the directories directly below
    /// their data sources' roots. Files at a root have none.
    pub top_directories: Vec<TopDirectory>,
    /// The assertions that the interpretation included about any of the
    /// files, newest first.
    pub assertions: Vec<Assertion>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Work {
    Movie {
        uuid: Uuid,
        title: String,
        year: Option<i64>,
        external_ids: Vec<ExternalId>,
        /// From a knowledge base.
        runtime_seconds: Option<i64>,
    },
    TvSeries {
        uuid: Uuid,
        title: String,
        /// The year its first episode aired.
        year: Option<i64>,
        external_ids: Vec<ExternalId>,
    },
}

/// A work's ID in a knowledge base.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExternalId {
    Imdb(String),
    Tmdb(u32),
    Tvdb(u32),
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct DataSource {
    pub uuid: Uuid,
    pub name: String,
}

/// A file, as an interpretation saw it.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct File {
    pub data_source: DataSource,
    /// Relative to the data source's root. Assertions name files by this and
    /// their data source.
    pub path: String,
    pub size_bytes: i64,
    /// What the file is in the catalog. A file can be several episodes.
    pub placements: Vec<Placement>,
    /// None when the file isn't media, or couldn't be probed.
    pub media: Option<Media>,
    /// Sorted by whether they're active, then by rank, highest first.
    pub claims: Vec<RankedClaim>,
    pub kb_queries: Vec<KbQuery>,
    /// The disc it stands for, if it's a disc's index file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disc: Option<Disc>,
    /// The index file of the disc it's one of the other files of, if it is
    /// one: assertions name that instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_of_disc: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Placement {
    Movie {
        uuid: Uuid,
        title: String,
        year: Option<i64>,
        /// None when nothing says what the file's role is.
        role: Option<FileRole>,
        /// For a main file, which of the movie's manifestations it's in, numbered
        /// from 0. A manifestation is one main file, or the parts of one, e.g. `CD1`
        /// and `CD2` beside each other.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        manifestation: Option<usize>,
        /// For a main file, which part of its manifestation it is, if its name
        /// numbers it as one, e.g. 1 for `CD1`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        part: Option<u32>,
    },
    Episode {
        series_uuid: Uuid,
        series_title: String,
        /// None when the season is unknown, and the episode is numbered
        /// through the whole series.
        season: Option<u32>,
        episode: u32,
        title: Option<String>,
    },
    SeriesExtra {
        series_uuid: Uuid,
        series_title: String,
    },
    /// A disc kept with the series, whose titles aren't its episodes.
    SeriesDisc {
        series_uuid: Uuid,
        series_title: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileRole {
    Main,
    Extra,
    /// A short clip cut from another file. Samples are left out of catalogs.
    Sample,
    /// Part of a disc, e.g. a Blu-ray's stream or playlist, whose index file is
    /// its movie's main file.
    Disc,
}

/// What probing a media file measured, in brief.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Media {
    /// The container format, e.g. `matroska,webm`.
    pub format: Option<String>,
    pub duration_seconds: Option<f64>,
    /// The container's title tag.
    pub title_tag: Option<String>,
    pub video: Vec<VideoStream>,
    pub audio: Vec<AudioStream>,
    pub subtitles: Vec<SubtitleStream>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct VideoStream {
    pub codec: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct AudioStream {
    pub codec: String,
    pub channels: u32,
    pub language: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct SubtitleStream {
    pub codec: String,
    pub language: Option<String>,
    pub forced: Option<bool>,
}

/// A claim about a file, as an interpretation ranked it.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct RankedClaim {
    pub claim: Claim,
    pub source: ClaimSource,
    /// Only comparable with other ranks in the same interpretation.
    pub rank: Rank,
    /// Whether it survived conflict resolution.
    pub active: bool,
}

/// How strongly a claim is believed: a claim outranks those it conflicts with
/// if its rank is greater. Assertions outrank every other claim, then the
/// knowledge base results that assertions found, then the rest by number.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Rank {
    /// A heuristic's claim, or of a knowledge base result a heuristic found.
    Number(f32),
    /// Of a knowledge base result an assertion found, e.g. an asserted ID's
    /// details, by how much the knowledge base favored it.
    FoundByAssertion(f32),
    /// An assertion's.
    Assertion,
}

/// What a heuristic, knowledge base or assertion says about a file. The
/// same claims can be asserted.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Claim {
    DurationSeconds(u64),
    FileRole(FileRole),
    MovieExternalId(ExternalId),
    MovieTitle(String),
    MovieReleaseYear(u32),
    TvSeriesExternalId(ExternalId),
    TvSeriesName(String),
    /// The year a series first aired, which tells it from another of its name.
    TvSeriesYear(u32),
    Season(u32),
    /// The first and last episode in the file, within its season.
    EpisodeRange([u32; 2]),
    /// The first and last episode in the file, numbered through the whole
    /// series.
    AbsoluteEpisodeRange([u32; 2]),
    /// The file is a special of its series, e.g. an OVA.
    Special,
    /// The file is part of its work, e.g. `Part Two`.
    Part(u32),
    /// The file's name marks it as a special of the series named. It's a
    /// movie if a knowledge base finds one, and otherwise a special.
    SpecialOf(String),
    /// The file is an extra of the work of the file at this path, in the same
    /// data source.
    ExtraOf(String),
    /// The file is part of the disc whose index file is at this path, in the
    /// same data source, e.g. a Blu-ray's stream, and so of its movie.
    DiscOf(String),
    /// The file isn't in the catalog, nor are the files that follow it, e.g.
    /// its disc's streams or its extras. Only ever asserted.
    Excluded,
    /// The file holds this edition of its work, e.g. `Director's Cut` or
    /// `Extended`. It says which cut the file is, not which work.
    Edition(String),
    /// The title of the episode the file is, e.g. `Ozymandias`.
    EpisodeTitle(String),
    /// Whether the file is a movie's or a series', before a knowledge base
    /// says which work it is.
    WorkKind(WorkKind),
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClaimSource {
    Heuristic {
        /// E.g. `release_name`. New heuristics are added over time.
        name: String,
    },
    Assertion,
    /// A knowledge base result. Claims with the same `result` come from one
    /// result, about one work, and stand or fall together.
    KnowledgeBase {
        result: usize,
    },
}

/// A query that interpretation made of a knowledge base about a file.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KbQuery {
    /// A work by another knowledge base's ID, e.g. an IMDb ID.
    Find {
        id: ExternalId,
        /// What it found, if anything, or if it was recorded.
        found: Option<KbLookupResult>,
    },
    MovieSearch {
        query: String,
        /// The first results, in the knowledge base's order.
        results: Vec<KbSearchResult>,
    },
    TvSeriesSearch {
        query: String,
        /// The first results, in the knowledge base's order.
        results: Vec<KbSearchResult>,
    },
    MovieDetails {
        id: ExternalId,
        /// The movie, if there's one by the ID, or if it was recorded.
        found: Option<KbLookupResult>,
    },
    TvSeriesDetails {
        id: ExternalId,
        /// The series, if there's one by the ID, or if it was recorded.
        found: Option<KbLookupResult>,
    },
    /// A season of a series, with its episodes, e.g. season 0, its specials.
    TvSeasonDetails { id: ExternalId, season: u32 },
    /// A movie's or a series' other titles, e.g. in other languages or
    /// romanized, by which its search may have found it.
    AlternativeTitles { id: ExternalId },
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct KbSearchResult {
    pub id: ExternalId,
    pub title: String,
    pub year: Option<u32>,
}

/// A work that a knowledge base lookup by its ID found.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct KbLookupResult {
    pub id: ExternalId,
    pub title: String,
    pub year: Option<u32>,
    /// How long it is, if it's a movie and the knowledge base says.
    pub runtime_minutes: Option<u32>,
}

/// A directory directly below a data source's root, e.g. `Title (2001)/`.
/// What the rest of a top directory became is often the best evidence of what a
/// file is.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct TopDirectory {
    pub data_source: DataSource,
    /// The directory itself.
    pub root: Directory,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Directory {
    pub name: String,
    /// Whether any of the work's own files are directly in it.
    pub has_work_files: bool,
    /// How many files are directly in it, or in its folded directories.
    pub file_count: usize,
    /// Where the files counted in `file_count` were placed, most first. A
    /// file with several placements counts in each.
    pub placements: Vec<PlacementCount>,
    /// The files directly in it, when asked for with `directory_files`.
    pub files: Option<Vec<DirectoryFile>>,
    /// Its subdirectories, unless they're folded.
    #[schema(no_recursion)]
    pub directories: Vec<Directory>,
    /// How many directories below it are folded into it: left out, with
    /// their files counted as its own. Unless `directory_files` is asked for,
    /// only the directories that lead to the work's files, and their
    /// subdirectories, are shown, and those subdirectories fold everything
    /// below them.
    pub folded_directories: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct PlacementCount {
    pub placement: PlacementGroup,
    pub files: usize,
}

/// Where a directory's files went in the catalog.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlacementGroup {
    Movie {
        uuid: Uuid,
        title: String,
        year: Option<i64>,
        role: Option<FileRole>,
    },
    Episodes {
        series_uuid: Uuid,
        series_title: String,
    },
    SeriesExtras {
        series_uuid: Uuid,
        series_title: String,
    },
    SeriesDiscs {
        series_uuid: Uuid,
        series_title: String,
    },
    /// Video files that no identified work includes, as the interpretation
    /// recorded them: what's left to identify.
    UnidentifiedVideos,
    /// Other files that are nothing in the catalog, e.g. subtitles or images.
    Other,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct DirectoryFile {
    pub name: String,
    pub size_bytes: i64,
    pub duration_seconds: Option<f64>,
    pub placements: Vec<Placement>,
}

/// A user's claim about a set of files, which every new interpretation
/// includes until it's retracted.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Assertion {
    pub uuid: Uuid,
    pub claim: Claim,
    pub files: Vec<AssertedFile>,
    /// UNIX timestamp, in seconds.
    pub created_at: i64,
    /// Why it was made.
    pub rationale: Option<String>,
    /// How sure its author was, from 0 to 1. It never affects belief.
    pub confidence: Option<f32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct AssertedFile {
    pub data_source: DataSource,
    pub path: String,
    /// UNIX timestamp, in seconds. Set when the assertion was retracted, or
    /// replaced for this file by a newer one of the same kind.
    pub retracted_at: Option<i64>,
}

/// `GET /api/v1/assertions`: the accepted assertions in effect, newest first,
/// which new interpretations include. It's the whole of what people and
/// agents have told WST, so it's also what `wst assertions export` writes and
/// `wst assertions import` reads.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct ActiveAssertions {
    pub assertions: Vec<Assertion>,
}

/// `POST /api/v1/assertions/import`: makes an export's assertions again, e.g.
/// in another instance, in a new changeset, all or none of them, oldest
/// first. Each file's data source is matched by UUID, or else by name, since
/// another instance's UUIDs mean nothing here; nothing is made if any is
/// missing. The changeset is left open, to review and accept.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct ImportAssertions {
    /// Who the changeset is by, e.g. "import".
    pub author: String,
    /// What it's for, e.g. which file it came from.
    #[serde(default)]
    pub description: String,
    /// As `GET /api/v1/assertions` lists them. Their UUIDs, and their files'
    /// retractions, are ignored.
    pub assertions: Vec<Assertion>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct ImportedAssertions {
    pub changeset: Uuid,
    /// In the order they were made.
    pub assertions: Vec<Uuid>,
}

/// `POST /api/v1/changesets`: opens a changeset, to make assertions and
/// retractions in, which take effect only once it's accepted.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct NewChangeset {
    /// Who makes it, e.g. "user", or an agent run's name.
    pub author: String,
    /// What it's for.
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChangesetState {
    /// Its changes are pending: only its own interpretations include them.
    Open,
    /// Its changes took effect.
    Accepted,
    /// Its pending changes were dropped.
    Discarded,
}

/// `GET /api/v1/changesets`
#[derive(Clone, Debug, Default, Deserialize, Serialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ChangesetsQuery {
    /// Only changesets in this state.
    #[param(inline)]
    pub state: Option<ChangesetState>,
}

/// Changesets, newest first.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Changesets {
    pub changesets: Vec<ChangesetSummary>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct ChangesetSummary {
    pub uuid: Uuid,
    pub author: String,
    pub description: String,
    pub state: ChangesetState,
    /// UNIX timestamp, in seconds.
    pub created_at: i64,
    /// When it was accepted or discarded. UNIX timestamp, in seconds.
    pub closed_at: Option<i64>,
    /// Not counting those withdrawn while it was open.
    pub assertion_count: u64,
    pub retraction_count: u64,
}

/// `GET /api/v1/changesets/{uuid}`
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Changeset {
    pub uuid: Uuid,
    pub author: String,
    pub description: String,
    pub state: ChangesetState,
    /// UNIX timestamp, in seconds.
    pub created_at: i64,
    /// When it was accepted or discarded. UNIX timestamp, in seconds.
    pub closed_at: Option<i64>,
    /// The assertions it makes, newest first, including those withdrawn
    /// while it was open, whose files are all retracted.
    pub assertions: Vec<Assertion>,
    /// The accepted assertions it retracts, or retracted.
    pub retractions: Vec<Uuid>,
}

/// `POST /api/v1/changesets/{uuid}/assertions`: makes assertions in an open
/// changeset, all or none of them. Nothing is interpreted.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct NewAssertions {
    pub assertions: Vec<NewAssertion>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct NewAssertion {
    /// The files the claim is about.
    pub files: Vec<FileRef>,
    pub claim: Claim,
    /// Why it's made, shown in audit logs.
    pub rationale: Option<String>,
    /// How sure its author is, from 0 to 1, for ordering reviews. It never
    /// affects belief.
    pub confidence: Option<f32>,
}

/// A file, by data source and path.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct FileRef {
    pub data_source: Uuid,
    /// Relative to the data source's root.
    pub path: String,
}

/// The assertions made, in the order they were asked for.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct CreatedAssertions {
    pub assertions: Vec<Uuid>,
}

/// `POST /api/v1/changesets/{uuid}/retractions`: retracts an assertion in an
/// open changeset. One of the changeset's own is withdrawn at once; an
/// accepted one is retracted once the changeset is accepted.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct NewRetraction {
    pub assertion: Uuid,
}

/// `POST /api/v1/changesets/{uuid}/accept`: applies an open changeset's
/// changes.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct AcceptedChangeset {
    /// The interpretation that is, or will be once it succeeds, the canonical
    /// one with the changeset's changes: the changeset's own if it was up to
    /// date, and otherwise a new one.
    pub interpretation: Uuid,
}

/// `GET /api/v1/diff`
#[derive(Clone, Debug, Deserialize, Serialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DiffQuery {
    /// The canonical interpretation by default.
    pub base: Option<Uuid>,
    pub other: Uuid,
}

/// What differs between two catalogs: `GET /api/v1/diff`, or
/// `POST /api/v1/changesets/{uuid}/diff` for what accepting a changeset would
/// change. Works are matched by the files they share; works that are the
/// same, with the same files placed the same way, are left out.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct CatalogDiff {
    pub base: Interpretation,
    pub other: Interpretation,
    /// Sorted by the works' titles.
    pub changes: Vec<WorkChange>,
    /// Video files that leave or join the unidentified videos, other than
    /// those of the changed works, e.g. files excluded from the catalog.
    #[serde(default)]
    pub unidentified: UnidentifiedChange,
}

/// Video files that leave or join the unidentified videos.
#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
pub struct UnidentifiedChange {
    /// Unidentified in the base catalog, but not in the other.
    pub left: Vec<DiffFile>,
    /// Unidentified in the other catalog, but not in the base.
    pub joined: Vec<DiffFile>,
}

/// A file, by data source and path.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct DiffFile {
    pub data_source: DataSource,
    pub path: String,
}

/// Works that share files, and how their files' placements differ.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct WorkChange {
    pub kind: WorkChangeKind,
    /// The works in the base catalog. Empty if they were added.
    pub before: Vec<DiffWork>,
    /// The works in the other catalog. Empty if they were removed.
    pub after: Vec<DiffWork>,
    /// The files whose placement differs, but for disc files.
    pub files: Vec<FileChange>,
    /// How many disc files' placements differ, e.g. a Blu-ray's streams.
    pub disc_files: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkChangeKind {
    /// Its files were in no work, e.g. unidentified.
    Added,
    /// Its files are in no work, e.g. unidentified.
    Removed,
    /// The same files, with different attributes, e.g. IDs, or placements.
    Changed,
    /// One work's files are several works'.
    Split,
    /// Several works' files are one work's.
    Merged,
    /// Several works' files are several other works'.
    Regrouped,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct DiffWork {
    pub kind: WorkKind,
    /// Only valid in its interpretation.
    pub uuid: Uuid,
    pub title: String,
    /// A movie's release year, or the year a series first aired.
    pub year: Option<i64>,
    pub external_ids: Vec<ExternalId>,
}

/// The kinds of work.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkKind {
    Movie,
    TvSeries,
}

/// A file's placements in each catalog. A file of several episodes has
/// several.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct FileChange {
    pub data_source: DataSource,
    pub path: String,
    pub before: Vec<DiffPlacement>,
    pub after: Vec<DiffPlacement>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct DiffPlacement {
    /// Index into the change's `before` or `after` works.
    pub work: usize,
    pub part: FilePart,
}

/// What a file is of its work.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FilePart {
    Movie {
        role: Option<FileRole>,
        /// For a main file, which of the movie's manifestations it's in, numbered
        /// from 0. A manifestation is one main file, or the parts of one, e.g. `CD1`
        /// and `CD2` beside each other.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        manifestation: Option<usize>,
        /// For a main file, which part of its manifestation it is, if its name
        /// numbers it as one, e.g. 1 for `CD1`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        part: Option<u32>,
    },
    Episode {
        season: Option<u32>,
        episode: u32,
    },
    SeriesExtra,
    /// A disc kept with the series, whose titles aren't its episodes.
    SeriesDisc,
}

/// `GET /api/v1/kb/search`
#[derive(Clone, Debug, Deserialize, Serialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct KbSearchQuery {
    #[param(inline)]
    pub kind: WorkKind,
    pub query: String,
    /// When a movie was released, or a series first aired.
    pub year: Option<u32>,
}

/// What a knowledge base search found, in the knowledge base's order. For
/// exploring: interpretation makes its own searches.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct KbSearchResults {
    pub results: Vec<KbSearchHit>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct KbSearchHit {
    pub id: ExternalId,
    pub title: String,
    pub original_title: String,
    /// When it was released, or a series first aired.
    pub year: Option<u32>,
    pub overview: String,
}

/// `GET /api/v1/kb/movies/{tmdb_id}`
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct KbMovie {
    pub ids: Vec<ExternalId>,
    pub title: String,
    pub original_title: String,
    pub original_language: String,
    pub year: Option<u32>,
    pub runtime_minutes: Option<u32>,
    pub overview: String,
}

/// `GET /api/v1/kb/tv-series/{tmdb_id}`
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct KbTvSeries {
    pub ids: Vec<ExternalId>,
    pub title: String,
    pub original_title: String,
    pub original_language: String,
    pub first_air_year: Option<u32>,
    pub last_air_year: Option<u32>,
    pub overview: String,
    /// With specials as season 0.
    pub seasons: Vec<KbSeason>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct KbSeason {
    pub number: u32,
    pub episode_count: u32,
    pub air_year: Option<u32>,
}

/// `GET /api/v1/marks`
#[derive(Clone, Debug, Default, Deserialize, Serialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MarksQuery {
    /// Also marks that are done.
    #[serde(default)]
    pub done: bool,
}

/// `GET /api/v1/marks`: what people marked as needing correction, newest
/// first.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Marks {
    pub marks: Vec<Mark>,
}

/// Files that someone noted need correcting, e.g. a misidentified movie's.
/// It's review state, not a claim: it never changes the catalog.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Mark {
    pub uuid: Uuid,
    /// What was marked, as the web UI named it, e.g. "movie Submarine D-1".
    pub label: String,
    /// Empty if none was given.
    pub note: String,
    /// UNIX timestamp, in seconds.
    pub created_at: i64,
    pub progress: MarkProgress,
    /// In order of their paths. A disc is named by its index file.
    pub files: Vec<MarkedFile>,
}

/// How far correcting a mark's files has got, from the assertions made about
/// them since it was made.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkProgress {
    /// Nothing asserted about its files yet.
    Open,
    /// An open changeset asserts about some of its files.
    Pending,
    /// Accepted assertions about all of its files were made since it was.
    /// That doesn't mean the catalog is right now; a diff says that.
    Done,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct MarkedFile {
    pub data_source: DataSource,
    /// Relative to the data source's root.
    pub path: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Claims are how clients will write assertions, so their JSON is the
    /// contract.
    #[test]
    fn claims_are_a_kind_and_a_value() {
        let cases = [
            (
                Claim::MovieExternalId(ExternalId::Tmdb(71883)),
                json!({"kind": "movie_external_id", "value": {"tmdb": 71883}}),
            ),
            (
                Claim::EpisodeRange([1, 2]),
                json!({"kind": "episode_range", "value": [1, 2]}),
            ),
            (Claim::Special, json!({"kind": "special"})),
        ];
        for (claim, expected) in cases {
            assert_eq!(serde_json::to_value(&claim).unwrap(), expected);
            assert_eq!(serde_json::from_value::<Claim>(expected).unwrap(), claim);
        }
    }
}

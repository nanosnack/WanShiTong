use crate::domain::{EpisodeNumber, Rank, SeasonNumber};
use crate::{CoreError, CoreErrorContext};
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct Ranked<T> {
    pub rank: Rank,
    pub value: T,
}

impl<T> Ranked<T> {
    pub fn new(rank: impl Into<Rank>, value: T) -> Self {
        Self {
            rank: rank.into(),
            value,
        }
    }
}

pub struct Claim {
    pub subject: SubjectIndex,
    pub pred: ClaimPredicate,
}
#[derive(Debug)]
pub struct RankedClaim {
    pub subject: SubjectIndex,
    pub pred: ClaimPredicate,
    pub rank: Rank,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RankedClaimPredicate {
    pub pred: ClaimPredicate,
    pub rank: Rank,
    /// All claims start active. They get deactivated during conflict resolution.
    pub active: bool,
    pub source: ClaimSource,
}

impl RankedClaimPredicate {
    pub fn new(source: ClaimSource, rank: impl Into<Rank>, pred: ClaimPredicate) -> Self {
        Self {
            pred,
            rank: rank.into(),
            active: true,
            source,
        }
    }

    /// The knowledge base result the claim came from, if any.
    pub fn kb_result(&self) -> Option<KbResultIndex> {
        match self.source {
            ClaimSource::KnowledgeBase { result } => Some(result),
            ClaimSource::Heuristic(_) | ClaimSource::Assertion => None,
        }
    }
}

/// Where a claim came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ClaimSource {
    Heuristic(Heuristic),
    Assertion,
    /// A knowledge base result. A result's claims are about one work, so they
    /// stand or fall together.
    KnowledgeBase {
        result: KbResultIndex,
    },
}

/// The heuristics that make claims about files.
///
/// Audit logs store these by position, so new ones go at the end.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Heuristic {
    /// A Blu-ray (BDMV) directory structure.
    Bdmv,
    /// A DVD (VIDEO_TS) directory structure.
    Dvd,
    /// A movie directory, with its main file and extras directories.
    MovieDirectory,
    /// A short clip named like a sample, or in a sample directory.
    Sample,
    /// The duration of the file's container.
    ContainerDuration,
    /// A video named like a movie release: a title, then its year.
    ReleaseName,
    /// A series name in the file name before what the file is of it, e.g.
    /// `Show Name` in `Show Name - S01E02` or `Show Name - NCOP 01`.
    FileNameSeriesName,
    /// A season and episode number, e.g. `S01E02`, in the file name.
    FileNameSeasonEpisode,
    /// The title tag of the file's container.
    ContainerTitleTag,
    /// A title, and maybe a release year, parsed from the file name.
    FileNameTitle,
    /// An extra's name, e.g. `NCOP`, or directory, e.g. `Extras`.
    ExtrasNameOrDirectory,
    /// The series of the episodes near an extra, e.g. in a sibling directory.
    SeriesOfNearbyEpisodes,
    /// The file name's title without a series' movie or OVA number, e.g.
    /// `Movie 01`.
    TitleWithoutNumbering,
    /// An episode number without a season in the file name, e.g. `E92` or
    /// `Episode 01`.
    FileNameEpisode,
    /// A season's directory, e.g. `Season 2`, of a file named with its
    /// episode number.
    SeasonDirectory,
    /// A number that varies between the names of videos in a directory,
    /// e.g. `01` in `Show Name - 01.mkv`, with `02` and `03` beside it.
    NumberedRun,
    /// An OVA marker in the file name, e.g. `Show Name S01 - OVA1`, and the
    /// series named before it.
    SeriesSpecial,
    /// A part number in the file name, e.g. `Part Two`.
    FileNamePart,
    /// A season release of a series, e.g. `Show.Name.S01.1080p`, that holds
    /// episodes named after it, and videos named after another series.
    SeasonRelease,
    /// The file name's title before its part, e.g. `Part 1`.
    TitleBeforePart,
    /// A number in the file name that's the season its directory names and
    /// an episode number, e.g. `730` in `Season 7/Show.Name.730.Title`.
    SeasonEpisodeNumber,
    /// A series name in the file name before a number that's the season its
    /// directory names and an episode, e.g. `Show Name` in
    /// `Season 5/Show.Name.515.Title.(E138)`, when the file's other marker,
    /// e.g. `E138`, gives another.
    SeriesNameBeforeSeasonEpisodeNumber,
    /// The series of the episodes near a video that's neither an episode nor
    /// an extra, e.g. `New Year Treat/` beside `Series 1/`, as a series it's a
    /// special of.
    SpecialOfNearbySeries,
    /// The file name's title without what its release adds, e.g. `M-01` in
    /// `M-01 Perry Mason - Perry Mason Returns`, or `USBD Remux`.
    TitleWithoutReleaseNoise,
    /// A number in a tag in the file name, e.g. `[01]`, that the videos named
    /// alike beside it have others of, as its part.
    PartInTag,
    /// An edition named in the file name, e.g. `Extended` or `Director's Cut`.
    FileNameEdition,
    /// The file name's title without the edition named in it, e.g. `DC
    /// Roadshow Version` in `Kingdom.of.Heaven.DC.Roadshow.Version.2005`.
    TitleWithoutEdition,
    /// The title after an episode's marker in the file name, e.g. `Alibi Me`
    /// in `Alfred Hitchcock Presents - S02E07 - Alibi Me`.
    FileNameEpisodeTitle,
    /// An ID written in the file's name or its directories', e.g. `{tmdb-8077}`
    /// or `[tvdbid-81797]`.
    NameExternalId,
    /// A number in parentheses after the file name's season and episode,
    /// e.g. `(130)` in `Show - S02E72 (130)`, that numbers the episode
    /// through the whole series, as the files beside it do alike.
    EpisodeInParentheses,
    /// The movie in the nearest directory above an extra that no series has,
    /// e.g. `Movie.mkv` for `PV/Movie PV 01.mkv`, as the movie it's of.
    ExtrasOfNearbyMovie,
    /// A number set apart in a lone video's name, e.g. `19` in `Show - 19`,
    /// as an episode of the series named before it.
    LoneNumberedEpisode,
    /// A title, or a series' name, cut before what seems its subtitle, e.g.
    /// `oh my jump!` for `oh my jump! ~shōnen jump chikyū wo sukū~`.
    TitleBeforeSubtitle,
    /// A title, or a series' name, with its words written in camel case
    /// split, e.g. `fuyu no sakura` for `FuyuNoSakura`.
    TitleSplitAtCapitals,
    /// A title, or a series' name, with long vowels written short, e.g.
    /// `umi no ue no shinryojo` for `Umi no Ue no Shinryoujo`.
    TitleWithShortVowels,
    /// A title, or a series' name, without the group's tag a lowercase
    /// scene name begins with, e.g. `red sonja` for `veto-red.sonja.2025`.
    TitleWithoutGroupTag,
    /// A title, or a series' name, without the words `and` and `plus`, which
    /// a file's name writes for `&` or `+`, e.g. `billy mandys big boogey
    /// adventure` for `Billy.and.Mandys.Big.Boogey.Adventure`.
    TitleWithoutAnd,
    /// A title cut to one word at its release's words, e.g. `aankhen` of
    /// `Aankhen.2002.1080p.AMZN.WEBDL`.
    TitleOfOneWord,
    /// Each name a season release gives its series, e.g. `my family` and
    /// `storia della mia famiglia` for `My Family AKA Storia della mia
    /// famiglia S02`.
    SeriesAlsoKnownAs,
    /// Whether a file is a movie's or a series': a series' if it's numbered
    /// as an episode, a movie's if it's a release's main file and says its
    /// year.
    KindOfWork,
    /// That a video with no kind of its own is a series', by the videos near
    /// it that have one.
    KindOfSiblings,
    /// That a video with no kind of its own is a movie's, as it runs as long
    /// as one.
    KindOfDuration,
    /// A series' name without the words at its end that a release adds,
    /// e.g. `detective conan` for `Detective Conan Remastered - 045`.
    SeriesNameWithoutReleaseWords,
    /// An NFO file's: a release's, with its IMDb link, or a media server's
    /// or its manager's, e.g. Kodi's XML, beside the video or at its series'
    /// root, with its work's IDs, title and year, and an episode's numbers.
    Nfo,
}

impl Heuristic {
    /// Whether its titles, or series' names, are only a last resort: cut,
    /// split or spelled otherwise from a file's, and looked up only if
    /// nothing else finds a work.
    pub fn is_last_resort_title(self) -> bool {
        matches!(
            self,
            Heuristic::TitleBeforeSubtitle
                | Heuristic::TitleOfOneWord
                | Heuristic::TitleSplitAtCapitals
                | Heuristic::TitleWithShortVowels
                | Heuristic::TitleWithoutGroupTag
                | Heuristic::TitleWithoutAnd
        )
    }
}

/// Index into the array of claim subjects.
pub type SubjectIndex = usize;

/// Index into the knowledge base results for one subject.
pub type KbResultIndex = usize;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ClaimPredicate {
    Duration(Duration),
    FileRole(FileRole),
    MovieExternalId(ExternalId),
    MovieTitle(String),
    MovieReleaseYear(u32),
    TvSeriesExternalId(ExternalId),
    TvSeriesName(String),
    /// The year a series first aired, e.g. 2011 for `Hunter x Hunter (2011)`,
    /// which tells it from another of its name, e.g. the 1999 series.
    TvSeriesYear(u32),
    Season(SeasonNumber),
    EpisodeRange([EpisodeNumber; 2]),
    /// The first and last episode in the file, numbered through the whole
    /// series rather than within a season, e.g. One Piece's episode 92.
    AbsoluteEpisodeRange([EpisodeNumber; 2]),
    /// The file is a special of its series, e.g. an OVA, rather than one of its
    /// numbered episodes.
    Special,
    /// The file is part of its work, e.g. `Part Two`: a sequel, half of a movie,
    /// or an episode, as a knowledge base may say.
    Part(u32),
    /// The file's name marks it as a special, e.g. an OVA, of the series
    /// named. It's a movie if a knowledge base finds one, and otherwise a
    /// special of the series.
    SpecialOf(String),
    /// The file is an extra of the work of the file at this path, in the same
    /// data source: of its movie, or of its series if it's an episode. It
    /// conflicts with any claim that the file is a work of its own.
    ExtraOf(String),
    /// The file is part of the disc whose index file is at this path, in the same
    /// data source, e.g. a Blu-ray's stream or playlist. No longer made, nor
    /// assertable: discs are found at ingest, and a disc's other files aren't
    /// subjects (see *Disc* in `spec/domain-model.md`). Old audit logs have it.
    DiscOf(String),
    /// The file isn't in the catalog, e.g. a sample or a file no knowledge base
    /// has, whatever it was taken for. Files that follow it, e.g. its disc's
    /// streams or its extras, aren't either. Only ever asserted.
    Excluded,
    /// The file holds this edition of its work, e.g. `Director's Cut` or
    /// `Extended`: which expression of the work it's a manifestation of (see
    /// *Expression* in `spec/domain-model.md`). It doesn't say which work.
    Edition(String),
    /// The title of the episode the file is, e.g. `Ozymandias`. A knowledge base
    /// may know the episode by it.
    EpisodeTitle(String),
    /// Whether the file is a movie's or a series', by what its name and its
    /// release say, before a knowledge base says which work it is. It shapes
    /// how the file's searched for, not which other claims hold.
    WorkKind(WorkKind),
}

/// A kind of work: a movie or a series.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub enum WorkKind {
    Movie,
    TvSeries,
}

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub enum FileRole {
    Main,
    Extra,
    /// A release sample: a short clip cut from another file. Samples are
    /// excluded from catalogs, so this role is never stored in `movie_files`.
    Sample,
    /// Part of a disc, e.g. a Blu-ray's stream or playlist, whose index file is
    /// its movie's main file. Only in catalogs from before discs were found at
    /// ingest, which stand for their files since.
    Disc,
}

impl FileRole {
    /// The role as stored in `movie_files.role`.
    pub(crate) fn as_db_str(self) -> &'static str {
        match self {
            FileRole::Main => "main",
            FileRole::Extra => "extra",
            FileRole::Sample => "sample",
            FileRole::Disc => "disc",
        }
    }

    pub(crate) fn from_db_str(s: &str) -> Result<Self, CoreError> {
        match s {
            "main" => Ok(FileRole::Main),
            "extra" => Ok(FileRole::Extra),
            "sample" => Ok(FileRole::Sample),
            "disc" => Ok(FileRole::Disc),
            unknown => Err(CoreErrorContext::UnknownFileRole(unknown.into()).into()),
        }
    }
}

impl ClaimPredicate {
    /// Whether two claims about one file can't both be true, so that the
    /// belief policy keeps only the higher-ranked: those of conflicting kinds
    /// (see [`ClaimKind::conflicts_with`]), and an extra's role and episode
    /// numbers, as an extra isn't an episode. Not a season, which an extra in
    /// a season's directory has too, e.g. `Season 1/NCED02.mkv`.
    pub fn conflicts_with(&self, other: &Self) -> bool {
        let extra_and_episode = |a: &Self, b: &Self| {
            matches!(a, Self::FileRole(FileRole::Extra))
                && matches!(b, Self::EpisodeRange(_) | Self::AbsoluteEpisodeRange(_))
        };
        self.kind().conflicts_with(other.kind())
            || extra_and_episode(self, other)
            || extra_and_episode(other, self)
    }

    pub fn kind(&self) -> ClaimKind {
        match self {
            Self::Duration(_) => ClaimKind::Duration,
            Self::FileRole(_) => ClaimKind::FileRole,
            Self::MovieExternalId(id) => match id {
                ExternalId::Imdb(_) => ClaimKind::MovieImdbId,
                ExternalId::Tmdb(_) => ClaimKind::MovieTmdbId,
                ExternalId::Tvdb(_) => ClaimKind::MovieTvdbId,
            },
            Self::MovieTitle(_) => ClaimKind::MovieTitle,
            Self::MovieReleaseYear(_) => ClaimKind::MovieReleaseYear,
            Self::TvSeriesExternalId(id) => match id {
                ExternalId::Imdb(_) => ClaimKind::TvSeriesImdbId,
                ExternalId::Tmdb(_) => ClaimKind::TvSeriesTmdbId,
                ExternalId::Tvdb(_) => ClaimKind::TvSeriesTvdbId,
            },
            Self::TvSeriesName(_) => ClaimKind::TvSeriesName,
            Self::TvSeriesYear(_) => ClaimKind::TvSeriesYear,
            Self::Season(_) => ClaimKind::Season,
            Self::EpisodeRange(_) => ClaimKind::EpisodeRange,
            Self::AbsoluteEpisodeRange(_) => ClaimKind::AbsoluteEpisodeRange,
            Self::Special => ClaimKind::Special,
            Self::Part(_) => ClaimKind::Part,
            Self::SpecialOf(_) => ClaimKind::SpecialOf,
            Self::ExtraOf(_) => ClaimKind::ExtraOf,
            Self::DiscOf(_) => ClaimKind::DiscOf,
            Self::Excluded => ClaimKind::Excluded,
            Self::Edition(_) => ClaimKind::Edition,
            Self::EpisodeTitle(_) => ClaimKind::EpisodeTitle,
            Self::WorkKind(_) => ClaimKind::WorkKind,
        }
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum ClaimKind {
    Duration,
    FileRole,
    MovieTmdbId,
    MovieImdbId,
    MovieTvdbId,
    MovieTitle,
    MovieReleaseYear,
    TvSeriesTmdbId,
    TvSeriesImdbId,
    TvSeriesTvdbId,
    TvSeriesName,
    TvSeriesYear,
    EpisodeRange,
    AbsoluteEpisodeRange,
    Special,
    Part,
    SpecialOf,
    Season,
    ExtraOf,
    DiscOf,
    Excluded,
    Edition,
    EpisodeTitle,
    WorkKind,
}

impl ClaimKind {
    fn media_work_type(self) -> Option<MediaWorkType> {
        match self {
            Self::Duration => None,
            Self::FileRole => None,
            Self::MovieTmdbId => Some(MediaWorkType::Movie),
            Self::MovieImdbId => Some(MediaWorkType::Movie),
            Self::MovieTvdbId => Some(MediaWorkType::Movie),
            Self::MovieTitle => Some(MediaWorkType::Movie),
            Self::MovieReleaseYear => Some(MediaWorkType::Movie),
            Self::TvSeriesTmdbId => Some(MediaWorkType::TvSeries),
            Self::TvSeriesImdbId => Some(MediaWorkType::TvSeries),
            Self::TvSeriesTvdbId => Some(MediaWorkType::TvSeries),
            Self::TvSeriesName => Some(MediaWorkType::TvSeries),
            Self::TvSeriesYear => Some(MediaWorkType::TvSeries),
            Self::EpisodeRange => Some(MediaWorkType::TvSeries),
            Self::AbsoluteEpisodeRange => Some(MediaWorkType::TvSeries),
            Self::Special => Some(MediaWorkType::TvSeries),
            // Facts a knowledge base looks up by, true of a movie and a series.
            Self::Part => None,
            Self::SpecialOf => None,
            Self::Season => Some(MediaWorkType::TvSeries),
            Self::ExtraOf => Some(MediaWorkType::ExtraOfAnother),
            Self::DiscOf => Some(MediaWorkType::ExtraOfAnother),
            Self::Excluded => None,
            // A cut of a movie or a series' episode alike.
            Self::Edition => None,
            // Looked up by once the series is found: it says nothing of whether
            // the file is a movie or an episode that its numbers don't.
            Self::EpisodeTitle => None,
            // What's guessed of the work's kind conflicts only with another
            // guess, so that the claims of the kind it isn't still hold.
            Self::WorkKind => None,
        }
    }

    pub fn conflicts_with(self, other: Self) -> bool {
        // Excluded from the catalog, a file is nothing in it, but it's still as
        // long as it was measured to be.
        if self == Self::Excluded || other == Self::Excluded {
            return self != Self::Duration && other != Self::Duration;
        }
        let self_mwt = self.media_work_type();
        let other_mwt = other.media_work_type();

        self == other || (self_mwt.is_some() && other_mwt.is_some() && self_mwt != other_mwt)
    }
}

#[derive(Eq, PartialEq)]
enum MediaWorkType {
    Movie,
    /// Part of another file's work, a movie or a series, rather than a work of
    /// its own.
    ExtraOfAnother,
    TvSeries,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExternalId {
    Imdb(String),
    Tmdb(u32),
    Tvdb(u32),
}

use std::path::PathBuf;
use uuid::Uuid;

/// Wan Shi Tong (WST) catalogs a media library.
///
/// `wst serve` runs the service. Every other command is a client of a running
/// service's JSON API, at `--url`.
///
/// You can use the client commands to discover unidentified files or
/// corrections that need to be made, and then create assertions to override the
/// invalid claims.
#[derive(clap::Parser)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(clap::Subcommand)]
pub enum Command {
    /// Run the WanShiTong service.
    Serve(ServeArgs),

    /// List unidentified media files.
    ///
    /// These are the media files that don't belong to any work in the catalog.
    ///
    /// `wst file <DATA_SOURCE> <PATH>` prints a file's evidence.
    Unidentified(UnidentifiedArgs),

    /// List what is marked as needing correction, newest first.
    ///
    /// Unidentified videos are only some of what needs correcting:
    /// misidentified works are found by people browsing the web UI, who mark
    /// their files, with an optional note saying what's wrong. Prints each
    /// mark's UUID, progress and label, its note, then its files by data
    /// source and path.
    ///
    /// A mark is:
    /// - "open" until assertions are made about its files
    /// - "pending" while an open changeset asserts about any of them
    /// - "done" once accepted assertions were made about all of them
    ///
    /// Done marks are left out, unless `--done` is given.
    Marks(MarksArgs),

    /// Print the evidence for a file, e.g. an unidentified video.
    ///
    /// Prints the file's relation to the catalog, what probing measured, the
    /// claims made about it, the knowledge base queries made about it, and info
    /// about its top directory.
    File(FileArgs),

    /// Print the evidence for a movie or TV series.
    ///
    /// Prints the work, then each of its files: what the catalog made of it,
    /// what probing measured, the claims made about it with their ranks and
    /// sources, and the knowledge base queries made about it with their top
    /// results. Then the top directories the files are in, and the assertions about
    /// any of the files.
    ///
    /// A top directory is a direct child of a data source directory, e.g. `Title
    /// (2001)/`. Its tree shows how many files each directory holds and where
    /// they went in the catalog; `*` marks directories with the work's own
    /// files. Only the directories that lead to the work's files are expanded;
    /// the others are summarized, e.g. `Scans/ (+3 directories)  40 files:
    /// ...`, unless `--directory-files` is given.
    ///
    /// Claims deactivated by conflict resolution are only counted, unless
    /// `--inactive` is given. A claim's rank only compares with other ranks in
    /// the same interpretation. Claims from one knowledge base result share its
    /// number, e.g. `kb#0`, and stand or fall together.
    Work(WorkArgs),

    /// Look things up in the knowledge bases (TMDB) that interpretation uses.
    ///
    /// Lookups go through the WST service, with its cache and credentials.
    /// They're for exploring what a work might be; they change nothing.
    #[command(subcommand)]
    Kb(KbCommand),

    /// Make and review changesets: assertions and retractions made together,
    /// which take effect only once accepted.
    ///
    /// Open one with `wst changeset new`, and set `WST_CHANGESET` to its UUID
    /// so that `wst assert` and `wst retract` make changes in it. Nothing is
    /// interpreted until the changeset is accepted. Accept it to apply to the
    /// canonical catalog, or discard it.
    #[command(subcommand)]
    Changeset(ChangesetCommand),

    /// Show what differs between two interpretations' catalogs.
    ///
    /// Compares `OTHER` with the canonical interpretation, or `--base`, as
    /// `wst changeset diff` does.
    Diff(DiffArgs),

    /// Assert a claim about files, in an open changeset.
    #[command(long_about = crate::claims::assert_about())]
    Assert(AssertArgs),

    /// Retract an assertion, in an open changeset.
    ///
    /// Retracting an assertion that was made in this changeset just drops it.
    /// Retracting an assertion in a separate accepted changeset will take effect
    /// once this changeset is accepted.
    Retract(RetractArgs),

    /// List, export and import the assertions in effect: the accepted
    /// assertions that new interpretations include.
    ///
    /// Everything else WST knows is measured from the files or fetched from
    /// knowledge bases, and can be made again; the assertions can't. `wst
    /// assertions export` backs them up, and `wst assertions import` makes
    /// them again in another instance.
    #[command(subcommand)]
    Assertions(AssertionsCommand),
}

#[derive(clap::Subcommand)]
pub enum AssertionsCommand {
    /// Print the assertions in effect, newest first.
    List(ClientArgs),

    /// Write the assertions in effect to a file, as JSON.
    ///
    /// Writes what `GET /api/v1/assertions` responds, pretty-printed: each
    /// assertion's claim, its files by data source and path, its rationale
    /// and confidence. That's enough for `wst assertions import` to make the
    /// same assertions in another instance.
    Export(ExportAssertionsArgs),

    /// Make the assertions in a file from `wst assertions export`, in a new
    /// changeset, and accept it.
    ///
    /// Each file's data source is matched by name, as another instance's
    /// UUIDs mean nothing there, so the data sources must exist first, with
    /// the same names; nothing is made if any is missing. The assertions are
    /// made oldest first, so they keep their order, then the changeset is
    /// accepted, unless `--open` leaves it to review, e.g. with `wst
    /// changeset diff`. The web UI's Assertions page imports the same way.
    ///
    /// Each assertion replaces any accepted one of the same kind about the
    /// same file, as accepting any changeset does, so importing into an
    /// instance that already has them leaves it as it was.
    Import(ImportAssertionsArgs),
}

#[derive(clap::Args)]
pub struct ExportAssertionsArgs {
    /// The WST service's URL, which it serves over plain HTTP.
    #[arg(long, env = "WST_URL", default_value = "http://localhost:6765")]
    pub url: String,

    /// The file to write, instead of standard output.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(clap::Args)]
pub struct ImportAssertionsArgs {
    #[command(flatten)]
    pub client: ClientArgs,

    /// The file to import, from `wst assertions export`.
    pub file: PathBuf,

    /// Who the changeset is by.
    #[arg(long, default_value = "import")]
    pub author: String,

    /// Leave the changeset open, to review before accepting it.
    #[arg(long)]
    pub open: bool,
}

#[derive(clap::Subcommand)]
pub enum ChangesetCommand {
    /// Open a changeset, and print its UUID.
    New(NewChangesetArgs),

    /// List changesets, newest first.
    List(ListChangesetsArgs),

    /// Print a changeset: its provenance, and the assertions and retractions
    /// it makes.
    Show(ChangesetArgs),

    /// Apply an open changeset's changes to the canonical catalog.
    ///
    /// Prints the interpretation that applies them, which becomes canonical
    /// once it succeeds.
    Accept(ChangesetArgs),

    /// Show what accepting an open changeset would change in the canonical
    /// catalog.
    ///
    /// Interprets the changeset first if it changed since it was last
    /// interpreted, which takes a few seconds. Prints each work that differs:
    /// added (its files were unidentified), removed (they will be), changed
    /// (e.g. its IDs, or its files' roles), split, merged, or regrouped, with
    /// the files whose placement differs. `+` marks where a file will be, `-`
    /// where it was. Disc files are counted, not listed.
    Diff(ChangesetArgs),

    /// Drop an open changeset's changes. It's kept, as history.
    Discard(ChangesetArgs),
}

#[derive(clap::Subcommand)]
pub enum KbCommand {
    /// Search for movies or TV series by title, and optionally year.
    ///
    /// Prints one result per line, in the knowledge base's order: its ID,
    /// title, original title if different, year, and the start of its
    /// overview.
    Search(KbSearchArgs),

    /// Print a movie's details by its TMDB ID, e.g. to compare its runtime
    /// with a file's duration.
    Movie(KbMovieArgs),

    /// Print a TV series' details by its TMDB ID, with its seasons' episode
    /// counts.
    TvSeries(KbTvSeriesArgs),
}

#[derive(clap::Args)]
pub struct KbSearchArgs {
    #[command(flatten)]
    pub client: ClientArgs,

    /// The title to search for.
    pub query: String,

    /// Search for TV series instead of movies.
    #[arg(long)]
    pub tv: bool,

    /// When the movie was released, or the series first aired.
    #[arg(long)]
    pub year: Option<u32>,
}

#[derive(clap::Args)]
pub struct KbMovieArgs {
    #[command(flatten)]
    pub client: ClientArgs,

    pub tmdb_id: u32,
}

#[derive(clap::Args)]
pub struct KbTvSeriesArgs {
    #[command(flatten)]
    pub client: ClientArgs,

    pub tmdb_id: u32,
}

/// The service's directories. Each defaults to the XDG base directory of its
/// kind, under the home directory, so that `wst serve` needs no flags; they're
/// created when missing.
#[derive(clap::Args)]
pub struct ServeArgs {
    /// Directory containing config.toml [default: $XDG_CONFIG_HOME/wst, i.e.
    /// ~/.config/wst]
    #[arg(short, long, env = "WST_CONFIG_DIR")]
    pub config_dir: Option<PathBuf>,

    /// Directory for the application database [default: $XDG_DATA_HOME/wst,
    /// i.e. ~/.local/share/wst]
    #[arg(short, long, env = "WST_DATA_DIR")]
    pub data_dir: Option<PathBuf>,

    /// Directory for the application cache [default: $XDG_CACHE_HOME/wst, i.e.
    /// ~/.cache/wst]
    #[arg(long, env = "WST_CACHE_DIR")]
    pub cache_dir: Option<PathBuf>,
}

/// How a client command reaches the service, and how it prints.
#[derive(clap::Args)]
pub struct ClientArgs {
    /// The WST service's URL, which it serves over plain HTTP.
    #[arg(long, env = "WST_URL", default_value = "http://localhost:6765")]
    pub url: String,

    /// Print the API's JSON response instead of text.
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct UnidentifiedArgs {
    #[command(flatten)]
    pub client: ClientArgs,

    /// Read this interpretation instead of the canonical one.
    #[arg(long)]
    pub interpretation: Option<Uuid>,

    /// Only files whose path contains this, ignoring case.
    #[arg(long)]
    pub name: Option<String>,

    /// Continue from a previous page's cursor.
    #[arg(long)]
    pub cursor: Option<String>,

    /// Files per page. The service caps it.
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
}

#[derive(clap::Args)]
pub struct MarksArgs {
    #[command(flatten)]
    pub client: ClientArgs,

    /// Also list marks that are done.
    #[arg(long)]
    pub done: bool,
}

#[derive(clap::Args)]
pub struct WorkArgs {
    #[command(flatten)]
    pub client: ClientArgs,

    /// The movie's or series' UUID, e.g. from `wst unidentified`.
    pub uuid: Uuid,

    /// Also print claims that lost in conflict resolution.
    #[arg(long)]
    pub inactive: bool,

    /// Show every directory of each top directory, and list their files.
    #[arg(long)]
    pub directory_files: bool,
}

#[derive(clap::Args)]
pub struct FileArgs {
    #[command(flatten)]
    pub client: ClientArgs,

    /// The UUID of the file's data source, e.g. from `wst unidentified`.
    pub data_source: Uuid,

    /// The file's path, relative to its data source's root.
    pub path: String,

    /// Read this interpretation instead of the canonical one.
    #[arg(long)]
    pub interpretation: Option<Uuid>,

    /// Also print claims that lost in conflict resolution.
    #[arg(long)]
    pub inactive: bool,

    /// Show the whole top directory, and list its files.
    #[arg(long)]
    pub directory_files: bool,
}

#[derive(clap::Args)]
pub struct NewChangesetArgs {
    #[command(flatten)]
    pub client: ClientArgs,

    /// Who makes it, e.g. an agent run's name.
    #[arg(long)]
    pub author: String,

    /// What it's for.
    #[arg(long, default_value = "")]
    pub description: String,
}

#[derive(clap::Args)]
pub struct ListChangesetsArgs {
    #[command(flatten)]
    pub client: ClientArgs,

    /// Only open changesets.
    #[arg(long)]
    pub open: bool,
}

#[derive(clap::Args)]
pub struct ChangesetArgs {
    #[command(flatten)]
    pub client: ClientArgs,

    /// The changeset's UUID.
    #[arg(env = "WST_CHANGESET")]
    pub changeset: Uuid,
}

#[derive(clap::Args)]
pub struct AssertArgs {
    #[command(flatten)]
    pub client: ClientArgs,

    /// The open changeset to assert in.
    #[arg(long, env = "WST_CHANGESET")]
    pub changeset: Uuid,

    /// A file the claim is about, by its data source's UUID and its path,
    /// e.g. from `wst unidentified`. Give it once for each file.
    #[arg(long, num_args = 2, value_names = ["DATA_SOURCE", "PATH"], required = true)]
    pub file: Vec<String>,

    /// Why, shown in audit logs, e.g. "TMDB 80809: runtime 96m vs file 95m".
    #[arg(long)]
    pub rationale: Option<String>,

    /// How sure, from 0 to 1, for ordering reviews. It never affects belief.
    #[arg(long)]
    pub confidence: Option<f32>,

    /// The claim: its kind, then its value.
    #[arg(required = true, num_args = 1.., trailing_var_arg = true, allow_hyphen_values = true)]
    pub claim: Vec<String>,
}

#[derive(clap::Args)]
pub struct RetractArgs {
    #[command(flatten)]
    pub client: ClientArgs,

    /// The open changeset to retract in.
    #[arg(long, env = "WST_CHANGESET")]
    pub changeset: Uuid,

    /// The assertion's UUID.
    pub assertion: Uuid,
}

#[derive(clap::Args)]
pub struct DiffArgs {
    #[command(flatten)]
    pub client: ClientArgs,

    /// The interpretation to compare.
    pub other: Uuid,

    /// The interpretation to compare it with, instead of the canonical one.
    #[arg(long)]
    pub base: Option<Uuid>,
}

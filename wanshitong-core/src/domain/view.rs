use crate::domain::{EpisodeNumber, FileRole, InterpretationRowId, SeasonNumber};
use derive_more::Display;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(
    Clone,
    Copy,
    Debug,
    Display,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
    Deserialize,
    Serialize,
    sqlx::Type,
)]
#[sqlx(transparent)]
pub struct ViewUuid(pub Uuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct ViewRowId(pub i64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct View {
    pub uuid: ViewUuid,
    pub name: String,
    pub created_at: Timestamp,
    pub config: ViewConfig,
    /// Whether the view is refreshed whenever another interpretation becomes
    /// canonical.
    pub auto_refresh: bool,
    pub latest_materialization: Option<ViewMaterialization>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewConfig {
    Jellyfin(JellyfinView),
    Plex(PlexView),
}

impl ViewConfig {
    pub fn kind(&self) -> ViewKind {
        match self {
            ViewConfig::Jellyfin(_) => ViewKind::Jellyfin,
            ViewConfig::Plex(_) => ViewKind::Plex,
        }
    }

    /// The directory the view's files are kept under.
    pub fn root_path(&self) -> &Path {
        match self {
            ViewConfig::Jellyfin(view) => &view.root_path,
            ViewConfig::Plex(view) => &view.root_path,
        }
    }

    /// The directory for the media server's movies library.
    pub fn movies_path(&self) -> PathBuf {
        match self {
            ViewConfig::Jellyfin(view) => view.movies_path(),
            ViewConfig::Plex(view) => view.movies_path(),
        }
    }

    /// The directory for the media server's TV shows library.
    pub fn shows_path(&self) -> PathBuf {
        match self {
            ViewConfig::Jellyfin(view) => view.shows_path(),
            ViewConfig::Plex(view) => view.shows_path(),
        }
    }
}

#[derive(Clone, Copy, Debug, Display, Eq, PartialEq)]
pub enum ViewKind {
    Jellyfin,
    Plex,
}

impl ViewKind {
    /// The kind as stored in `views.kind`.
    pub(crate) fn as_db_str(self) -> &'static str {
        match self {
            ViewKind::Jellyfin => "jellyfin",
            ViewKind::Plex => "plex",
        }
    }
}

/// A tree of symlinks to media files, named by Jellyfin's rules.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JellyfinView {
    pub root_path: PathBuf,
}

impl JellyfinView {
    /// The directory for a Jellyfin movies library.
    ///
    /// Jellyfin wants movies and shows in separate libraries, so each gets its
    /// own directory under the root.
    pub fn movies_path(&self) -> PathBuf {
        self.root_path.join("Movies")
    }

    /// The directory for a Jellyfin shows library.
    pub fn shows_path(&self) -> PathBuf {
        self.root_path.join("Shows")
    }
}

/// A tree of symlinks to media files, named by Plex's rules.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlexView {
    pub root_path: PathBuf,
}

impl PlexView {
    /// The directory for a Plex movies library.
    ///
    /// Plex wants movies and TV shows in separate libraries, so each gets its
    /// own directory under the root.
    pub fn movies_path(&self) -> PathBuf {
        self.root_path.join("Movies")
    }

    /// The directory for a Plex TV shows library.
    pub fn shows_path(&self) -> PathBuf {
        self.root_path.join("TV Shows")
    }
}

/// One attempt to bring a view up to date with an interpretation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewMaterialization {
    /// `None` if the interpretation has since been deleted.
    pub interpretation_id: Option<InterpretationRowId>,
    pub started_at: Timestamp,
    pub stopped_at: Timestamp,
    /// The error message on failure.
    pub result: Result<ViewSyncStats, String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ViewSyncStats {
    pub links_created: u64,
    pub links_updated: u64,
    pub links_removed: u64,
}

/// A movie in the shape a view needs to name and link its files.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewMovie {
    pub title: String,
    pub release_year: Option<u32>,
    pub tmdb_id: Option<u32>,
    pub imdb_id: Option<String>,
    pub files: Vec<ViewMovieFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewMovieFile {
    /// None when nothing says what the file's role is.
    pub role: Option<FileRole>,
    /// For a main file, which of the movie's manifestations it's in, numbered from 0.
    /// See *Manifestation* in `spec/domain-model.md`.
    pub manifestation: Option<usize>,
    /// For a main file, which part of its manifestation it is, if its name numbers it
    /// as one, e.g. 1 for `CD1`. A manifestation of several files has one for each.
    pub part: Option<u32>,
    /// For a main file, the edition of the expression its manifestation realizes, e.g.
    /// `Director's Cut`, or None for the unspecified expression.
    pub edition: Option<String>,
    /// Whether it's a disc's index file, e.g. a disc folder's `index.bdmv` or
    /// a disc image, which stands for the whole disc.
    pub disc: bool,
    /// Absolute path: the data source's root joined with the file's relative
    /// path.
    pub path: PathBuf,
}

/// A TV series in the shape a view needs to name and link its files.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewTvSeries {
    pub name: String,
    pub tmdb_id: Option<u32>,
    pub tvdb_id: Option<u32>,
    pub imdb_id: Option<String>,
    /// Each episode in each of its files, so a double episode's file is here
    /// once per episode.
    pub episode_files: Vec<ViewEpisodeFile>,
    /// Absolute paths of the files that belong to the series without being
    /// episodes of it.
    pub extras: Vec<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewEpisodeFile {
    /// None for an episode that nothing says the season of, which is numbered
    /// through the whole series.
    pub season: Option<SeasonNumber>,
    pub episode: EpisodeNumber,
    /// Absolute path: the data source's root joined with the file's relative
    /// path.
    pub path: PathBuf,
}

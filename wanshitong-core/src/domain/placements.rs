use crate::domain::{DataSourceUuid, FileRole};
use std::path::PathBuf;
use uuid::Uuid;

/// Where an interpretation's catalog put each of the files it placed, and
/// the works it put them in: what comparing two catalogs needs.
#[derive(Clone, Debug, Default)]
pub struct CatalogPlacements {
    pub works: Vec<CatalogWork>,
    /// One per file and placement: a file of several episodes is placed in
    /// each.
    pub files: Vec<PlacedFile>,
    /// The video files in no identified work, and not excluded: what's left
    /// to identify.
    pub unidentified: Vec<CatalogFile>,
}

/// A file, by data source and path.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CatalogFile {
    pub data_source_uuid: DataSourceUuid,
    pub data_source_name: String,
    pub rel_path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogWork {
    pub kind: CatalogWorkKind,
    /// Only valid in its interpretation.
    pub uuid: Uuid,
    pub title: String,
    /// A movie's release year, or the year a series first aired.
    pub year: Option<i64>,
    pub tmdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CatalogWorkKind {
    Movie,
    TvSeries,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacedFile {
    pub data_source_uuid: DataSourceUuid,
    pub data_source_name: String,
    pub rel_path: PathBuf,
    /// Index into [`CatalogPlacements::works`].
    pub work: usize,
    pub part: FilePart,
}

/// What a file is of its work.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FilePart {
    /// A movie's file.
    Movie {
        /// None when nothing says what the file's role is.
        role: Option<FileRole>,
        /// For a main file, which of the movie's manifestations it's in, numbered
        /// from 0. See *Manifestation* in `spec/domain-model.md`.
        manifestation: Option<usize>,
        /// For a main file, which part of its manifestation it is, if its name
        /// numbers it as one, e.g. 1 for `CD1`.
        part: Option<u32>,
    },
    /// An episode of a series, in a season unless it's unknown.
    Episode {
        season: Option<u32>,
        episode: u32,
    },
    SeriesExtra,
    /// A disc kept with the series, which isn't one of its episodes.
    SeriesDisc,
}

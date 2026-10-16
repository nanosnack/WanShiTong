use crate::domain::{DataSourceUuid, FileMediaMeasurements, FileRole, IngestUuid, SubjectAuditLog};
use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};
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
pub struct MovieUuid(pub Uuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sqlx::Type, Deserialize, Serialize)]
#[sqlx(transparent)]
pub struct MovieRowId(pub i64);

pub struct MovieSummary {
    pub id: MovieRowId,
    pub uuid: MovieUuid,
    pub title: String,
    pub release_year: Option<i64>,
    pub tmdb_id: Option<i64>,
    pub tmdb_poster_path: Option<String>,
}

pub struct MovieDetails {
    pub id: MovieRowId,
    pub uuid: MovieUuid,
    pub title: String,
    pub tmdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i64>,

    pub original_title: Option<String>,
    pub original_language: Option<String>,
    pub release_year: Option<i64>,
    pub tagline: Option<String>,
    pub overview: Option<String>,
    pub runtime_seconds: Option<i64>,
    pub tmdb_poster_path: Option<String>,

    pub data_source_uuid: DataSourceUuid,
    pub data_source_name: String,
    pub rel_path: PathBuf,

    pub ingest_uuid: IngestUuid,

    // TODO: shouldn't this always exist?
    pub media: Option<FileMediaMeasurements>,

    /// All files associated with this movie.
    pub files: Vec<MovieFile>,
}

impl MovieDetails {
    pub fn duration(&self) -> Option<Duration> {
        if let Some(media) = &self.media {
            media.container.as_ref()?.duration()
        } else {
            self.runtime_seconds
                .map(|runtime_seconds| Duration::from_secs(runtime_seconds as u64))
        }
    }
}

pub struct MovieFile {
    pub data_source_uuid: DataSourceUuid,
    pub rel_path: PathBuf,
    /// None when nothing says what the file's role is.
    pub role: Option<FileRole>,
    pub audit_log: SubjectAuditLog,
}

/// Semantic data about a movie that we only get from an external knowledge base.
///
/// This excludes data that was already emitted via claims during movie
/// identification.
#[derive(Debug, Default)]
pub struct MovieKbData {
    pub original_title: Option<String>,
    pub original_language: Option<String>,
    pub tagline: Option<String>,
    pub overview: Option<String>,
    pub release_date: Option<Date>,
    pub poster_path: Option<String>,
    pub runtime: Option<Duration>,
}
/// A calendar date. Its fields, in order, order dates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: u32,
    pub month: u32,
    pub day: u32,
}

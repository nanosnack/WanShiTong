use crate::domain::{DataSourceUuid, Date, SubjectAuditLog};
use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub type SeasonNumber = u32;
pub type EpisodeNumber = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sqlx::Type, Deserialize, Serialize)]
#[sqlx(transparent)]
pub struct TvSeriesRowId(pub i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct TvSeasonRowId(pub i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct TvEpisodeRowId(pub i64);

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
pub struct TvSeriesUuid(pub Uuid);

pub struct TvSeriesSummary {
    pub id: TvSeriesRowId,
    pub uuid: TvSeriesUuid,
    pub name: String,
    pub tmdb_id: Option<i64>,
    pub tmdb_poster_path: Option<String>,
    /// Not counting the episodes of an unknown season.
    pub season_count: i64,
    pub episode_count: i64,
}

pub struct TvSeriesDetails {
    pub uuid: TvSeriesUuid,
    pub name: String,
    pub original_name: Option<String>,
    pub tmdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i64>,
    pub original_language: Option<String>,
    /// The year its first episode aired.
    pub first_air_year: Option<i64>,
    pub tagline: Option<String>,
    pub overview: Option<String>,
    pub tmdb_poster_path: Option<String>,
    /// The data sources its files are in, in order of their names.
    pub data_sources: Vec<DataSourceRef>,
    /// In order of their numbers, then the episodes of an unknown season.
    pub seasons: Vec<TvSeason>,
    /// Files that belong to the series without being episodes of it, e.g.
    /// creditless openings, in order of their paths.
    pub extras: Vec<TvSeriesFile>,
    /// Its discs, by their index files: kept with it, but not its episodes.
    /// See *Disc* in `spec/domain-model.md`.
    pub discs: Vec<TvSeriesFile>,
}

pub struct TvSeason {
    /// None for the series' episodes that nothing says the season of, which
    /// are numbered through the whole series.
    pub number: Option<SeasonNumber>,
    /// In order of their numbers.
    pub episodes: Vec<TvEpisode>,
}

pub struct TvEpisode {
    pub number: EpisodeNumber,
    pub title: Option<String>,
    /// In order of their paths.
    pub files: Vec<TvSeriesFile>,
}

/// A file of an episode or an extra of a series. A file with several episodes
/// is a file of each.
pub struct TvSeriesFile {
    pub data_source_uuid: DataSourceUuid,
    pub rel_path: PathBuf,
    pub audit_log: SubjectAuditLog,
}

/// Data about a TV series that we only get from an external knowledge base.
#[derive(Debug, Default)]
pub struct TvSeriesKbData {
    pub original_name: Option<String>,
    pub original_language: Option<String>,
    pub tagline: Option<String>,
    pub overview: Option<String>,
    pub first_air_date: Option<Date>,
    pub poster_path: Option<String>,
}

/// A data source, by what names it: its UUID and its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataSourceRef {
    pub uuid: DataSourceUuid,
    pub name: String,
}

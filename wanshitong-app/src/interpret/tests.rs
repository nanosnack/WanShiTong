mod assertions;
mod discs;
mod episodes;
mod errors;
mod extras;
mod movies;
mod nfo;
mod parts;
mod runs;
mod series;
mod specials;

use super::*;
use knowledge_base::{
    Episode, KbResult, KnowledgeBaseApi, MovieRecord, RankedSearchFields, Season, SeasonSummary,
    SeriesRecord,
};
use std::{path::PathBuf, time::Duration};
use uuid::Uuid;
use wanshitong_core::{
    commands::{NewCatalog, NewMovie, NewMovieExpression},
    domain::{
        AssertedFile, Assertion, AssertionUuid, ClaimPredicate, ContainerMeasurement,
        DataSourceUuid, DiscFacts, DiscKind, ExternalId, FileFacts, FileMeasurement,
        FileMeasurementRowId, FileMediaMeasurements, FileRole, Heuristic, KbQueryEvent,
        MovieKbData, PathId, TvSeriesKbData, VideoStreamMeasurement, WorkKind,
    },
};

/// The interpretation pipeline, minus the knowledge base.
fn catalog_without_kb(files: &[(PathId, FileFacts)], assertions: &[Assertion]) -> NewCatalog {
    let mut claims = run_heuristics(files);
    add_assertion_claims(files, assertions, &mut claims);
    deactivate_duplicate_claims(&mut claims);
    apply_belief_policy(&mut claims);
    let audit_logs = AuditLogger::default().finish(claims.clone());
    new_catalog(files, build_works(files, claims), audit_logs)
}

/// A file that ffprobe couldn't read.
fn file(row_id: i64, rel_path: &str) -> (PathId, FileFacts) {
    let path_id = PathId {
        data_source: DataSourceUuid(Uuid::nil()),
        rel_path: PathBuf::from(rel_path),
    };
    let facts = FileFacts {
        row_id: FileMeasurementRowId(row_id),
        file: FileMeasurement {
            device: None,
            inode: None,
            size_bytes: 0,
            mtime: 0,
            ctime: None,
            hardlink_count: None,
            is_symlink: false,
            media_measurement_id: None,
        },
        mime_type: None,
        media: None,
        disc: None,
        nfo_text: None,
    };
    (path_id, facts)
}

/// An NFO file, with `text` in it.
fn nfo(row_id: i64, rel_path: &str, text: &str) -> (PathId, FileFacts) {
    let (path_id, mut facts) = file(row_id, rel_path);
    facts.nfo_text = Some(text.into());
    (path_id, facts)
}

fn video(row_id: i64, rel_path: &str, minutes: f64) -> (PathId, FileFacts) {
    let (path_id, mut facts) = file(row_id, rel_path);
    facts.media = Some(FileMediaMeasurements {
        container: Some(ContainerMeasurement {
            format: "matroska,webm".into(),
            duration_ms: Some((minutes * 60_000.0) as i64),
            bitrate: None,
            stream_count: 1,
            tags: None,
        }),
        video: vec![VideoStreamMeasurement {
            stream_index: 0,
            codec: "h264".into(),
            profile: None,
            width: 1920,
            height: 1080,
            pixel_format: None,
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            color_range: None,
            frame_rate_num: None,
            frame_rate_den: None,
            bit_depth: None,
            bitrate: None,
            duration_ms: None,
            tags: None,
            side_data_list: vec![],
        }],
        audio: vec![],
        text: vec![],
        unclassified: vec![],
        disc_image: None,
    });
    (path_id, facts)
}

/// A movie's title, and its files' row IDs and roles.
type MovieByTitle = (String, Vec<(i64, Option<FileRole>)>);

/// Each movie, sorted by title.
fn movies(catalog: &NewCatalog) -> Vec<MovieByTitle> {
    let mut movies: Vec<_> = catalog
        .movies
        .iter()
        .map(|NewMovie { title, files, .. }| {
            let mut files: Vec<_> = files.iter().map(|f| (f.file_id.0, f.role)).collect();
            files.sort_by_key(|&(id, _)| id);
            (title.clone(), files)
        })
        .collect();
    movies.sort_by(|a, b| a.0.cmp(&b.0));
    movies
}

/// A disc's index file, which stands for the disc: its other files never
/// reach interpretation.
fn disc(
    row_id: i64,
    index_path: &str,
    kind: DiscKind,
    root: &str,
    minutes: f64,
) -> (PathId, FileFacts) {
    let (path_id, mut facts) = file(row_id, index_path);
    facts.disc = Some(DiscFacts {
        kind,
        root: root.into(),
        file_count: 10,
        size_bytes: 0,
        duration: Some(Duration::from_millis((minutes * 60_000.0) as u64)),
    });
    (path_id, facts)
}

fn assertion(data_source: DataSourceUuid, rel_paths: &[&str], claim: ClaimPredicate) -> Assertion {
    Assertion {
        uuid: AssertionUuid(Uuid::new_v4()),
        claim,
        files: rel_paths
            .iter()
            .map(|rel_path| AssertedFile {
                data_source_uuid: data_source,
                data_source_name: "media".into(),
                rel_path: rel_path.into(),
                retracted_at: None,
            })
            .collect(),
        created_at: Default::default(),
        rationale: None,
        confidence: None,
    }
}

/// The interpretation pipeline, with a knowledge base.
async fn catalog_with_kb(
    files: &[(PathId, FileFacts)],
    assertions: &[Assertion],
    kb: &dyn KnowledgeBase,
) -> NewCatalog {
    let mut logger = AuditLogger::default();
    let mut claims = run_heuristics(files);
    add_assertion_claims(files, assertions, &mut claims);
    deactivate_duplicate_claims(&mut claims);
    add_claims_from_knowledge_base(kb, files, &mut logger, &mut claims, &mut Vec::new())
        .await
        .unwrap();
    apply_belief_policy(&mut claims);
    let audit_logs = logger.finish(claims.clone());
    new_catalog(files, build_works(files, claims), audit_logs)
}

/// Answers every search with the same results.
struct FixedKb(Vec<KbResult>);

#[async_trait::async_trait]
impl KnowledgeBase for FixedKb {
    async fn search_for_claims(
        &self,
        _fields: &RankedSearchFields,
    ) -> Result<(Vec<KbResult>, Vec<KbQueryEvent>), KnowledgeBaseError> {
        Ok((self.0.clone(), vec![]))
    }

    async fn get_movie_details(&self, _id: u32) -> Result<Option<MovieKbData>, KnowledgeBaseError> {
        Ok(None)
    }

    async fn get_tv_series_details(
        &self,
        _id: u32,
    ) -> Result<Option<TvSeriesKbData>, KnowledgeBaseError> {
        Ok(None)
    }
}

/// A movie's TMDB ID, and its files' row IDs and roles.
type MovieById = (Option<u32>, Vec<(i64, Option<FileRole>)>);

fn movies_by_id(catalog: &NewCatalog) -> Vec<MovieById> {
    let mut movies: Vec<_> = catalog
        .movies
        .iter()
        .map(|movie| {
            let mut files: Vec<_> = movie.files.iter().map(|f| (f.file_id.0, f.role)).collect();
            files.sort_by_key(|&(id, _)| id);
            (movie.tmdb_id, files)
        })
        .collect();
    movies.sort_by_key(|(tmdb_id, files)| (*tmdb_id, files.first().map(|&(id, _)| id)));
    movies
}

/// Each series' name, and its episodes' row IDs, seasons and episodes.
type SeriesEpisodes = (String, Vec<(i64, Option<u32>, [u32; 2])>);

fn series_episodes(catalog: &NewCatalog) -> Vec<SeriesEpisodes> {
    let mut series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let mut episodes: Vec<_> = series
                .episodes
                .iter()
                .map(|e| (e.file_id.0, e.season_number, e.episode_range))
                .collect();
            episodes.sort();
            (series.name.clone(), episodes)
        })
        .collect();
    series.sort();
    series
}

/// A knowledge base of fixed movies, series and seasons, searched as TMDB
/// is, by [`knowledge_base::search`]. Its searches find, in order, what's
/// named like the query, and of the year searched for, if one is.
#[derive(Default)]
struct CatalogKb {
    movies: Vec<MovieRecord>,
    series: Vec<SeriesRecord>,
    /// Seasons, by their series' TMDB ID.
    seasons: Vec<(u32, Season)>,
    /// Other titles of movies and series, by their TMDB IDs, by which a
    /// search finds them too.
    alternative_titles: Vec<(u32, &'static str)>,
}

impl CatalogKb {
    fn alternative_titles_of(&self, id: &ExternalId) -> Vec<String> {
        self.alternative_titles
            .iter()
            .filter(|(of, _)| ExternalId::Tmdb(*of) == *id)
            .map(|(_, title)| title.to_string())
            .collect()
    }

    /// Whether a work is named like `query`, by its title or another.
    fn titled_like(&self, id: &ExternalId, title: &str, query: &str) -> bool {
        named_like(title, query)
            || self
                .alternative_titles_of(id)
                .iter()
                .any(|other| named_like(other, query))
    }
}

/// A knowledge base of series, from a list of (TMDB ID, name, year first
/// aired).
fn series_kb(series: Vec<(u32, &str, u32)>) -> CatalogKb {
    CatalogKb {
        series: series
            .into_iter()
            .map(|(id, name, year)| kb_series(id, name, Some(year)))
            .collect(),
        ..CatalogKb::default()
    }
}

fn kb_movie(id: u32, title: &str, year: Option<u32>, runtime_minutes: Option<u32>) -> MovieRecord {
    MovieRecord {
        id: ExternalId::Tmdb(id),
        title: title.into(),
        original_title: title.into(),
        release_year: year,
        runtime_minutes,
        imdb_id: None,
    }
}

fn kb_series(id: u32, name: &str, first_air_year: Option<u32>) -> SeriesRecord {
    SeriesRecord {
        id: ExternalId::Tmdb(id),
        name: name.into(),
        original_name: name.into(),
        first_air_year,
        seasons: Vec::new(),
    }
}

/// Whether a search for `query` finds a work named `name`: whether the name
/// has all the query's words, e.g. "Code Geass: Hangyaku no Lelouch -
/// Nunnally in Wonderland" for `code geass - nunnally in wonderland`.
fn named_like(name: &str, query: &str) -> bool {
    crate::title::coverage(name, query) == 1.0
}

#[async_trait::async_trait]
impl KnowledgeBaseApi for CatalogKb {
    async fn movie(
        &self,
        id: &ExternalId,
        _log: &mut Vec<KbQueryEvent>,
    ) -> Result<Option<MovieRecord>, KnowledgeBaseError> {
        Ok(self.movies.iter().find(|movie| movie.id == *id).cloned())
    }

    async fn series(
        &self,
        id: &ExternalId,
        _log: &mut Vec<KbQueryEvent>,
    ) -> Result<Option<SeriesRecord>, KnowledgeBaseError> {
        let Some(series) = self.series.iter().find(|series| series.id == *id) else {
            return Ok(None);
        };
        // A series' details list its seasons, unlike a search's results.
        let seasons = self
            .seasons
            .iter()
            .filter(|(of, _)| ExternalId::Tmdb(*of) == *id)
            .map(|(_, season)| SeasonSummary {
                number: season.number,
                name: format!("Season {}", season.number),
                episode_count: season.episodes.len() as u32,
            });
        Ok(Some(SeriesRecord {
            seasons: series.seasons.iter().cloned().chain(seasons).collect(),
            ..series.clone()
        }))
    }

    async fn season(
        &self,
        series: &ExternalId,
        number: u32,
        _log: &mut Vec<KbQueryEvent>,
    ) -> Result<Option<Season>, KnowledgeBaseError> {
        Ok(self
            .seasons
            .iter()
            .find(|(id, season)| ExternalId::Tmdb(*id) == *series && season.number == number)
            .map(|(_, season)| season.clone()))
    }

    async fn search_movies(
        &self,
        query: &str,
        year: Option<u32>,
    ) -> Result<Vec<MovieRecord>, KnowledgeBaseError> {
        Ok(self
            .movies
            .iter()
            .filter(|movie| self.titled_like(&movie.id, &movie.title, query))
            // As TMDB's, by any of a movie's release dates, which may be a
            // year off its first, e.g. a festival's and a release's.
            .filter(|movie| {
                year.is_none_or(|year| movie.release_year.is_some_and(|y| y.abs_diff(year) <= 1))
            })
            .cloned()
            .collect())
    }

    async fn search_series(
        &self,
        query: &str,
        year: Option<u32>,
    ) -> Result<Vec<SeriesRecord>, KnowledgeBaseError> {
        Ok(self
            .series
            .iter()
            .filter(|series| self.titled_like(&series.id, &series.name, query))
            .filter(|series| year.is_none_or(|year| series.first_air_year == Some(year)))
            // Without their seasons, as a search's results don't list them.
            .map(|series| SeriesRecord {
                seasons: Vec::new(),
                ..series.clone()
            })
            .collect())
    }

    async fn movie_alternative_titles(
        &self,
        id: &ExternalId,
        _log: &mut Vec<KbQueryEvent>,
    ) -> Result<Vec<String>, KnowledgeBaseError> {
        Ok(self.alternative_titles_of(id))
    }

    async fn series_alternative_titles(
        &self,
        id: &ExternalId,
        _log: &mut Vec<KbQueryEvent>,
    ) -> Result<Vec<String>, KnowledgeBaseError> {
        Ok(self.alternative_titles_of(id))
    }

    async fn get_movie_details(&self, _id: u32) -> Result<Option<MovieKbData>, KnowledgeBaseError> {
        Ok(None)
    }

    async fn get_tv_series_details(
        &self,
        _id: u32,
    ) -> Result<Option<TvSeriesKbData>, KnowledgeBaseError> {
        Ok(None)
    }
}

fn active_claims(catalog: &NewCatalog, file_id: i64) -> Vec<ClaimPredicate> {
    let (_, log) = catalog
        .audit_logs
        .iter()
        .find(|(id, _)| id.0 == file_id)
        .unwrap();
    log.claims
        .iter()
        .filter(|claim| claim.active)
        .map(|claim| claim.pred.clone())
        .collect()
}

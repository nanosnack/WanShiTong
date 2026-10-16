use crate::interpret::knowledge_base::{
    Episode, KnowledgeBaseApi, KnowledgeBaseError, MovieRecord, Season, SeasonSummary, SeriesRecord,
};
use crate::tmdb_client::{
    FindId, MovieDetails, MovieSummary, SeasonDetails, TmdbClient, TvDetails, TvSummary, year_of,
};
use anyhow::anyhow;
use async_trait::async_trait;
use std::time::Duration;
use wanshitong_core::domain::{
    Date, ExternalId, KbLookupResult, KbQueryEvent, MovieKbData, TvSeriesKbData,
};

pub struct Tmdb {
    client: TmdbClient,
}

impl Tmdb {
    pub fn new(client: TmdbClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl KnowledgeBaseApi for Tmdb {
    /// A TMDB ID's movie, or the first that TMDB finds by another's ID.
    async fn movie(
        &self,
        id: &ExternalId,
        log: &mut Vec<KbQueryEvent>,
    ) -> Result<Option<MovieRecord>, KnowledgeBaseError> {
        let find = match id {
            ExternalId::Imdb(imdb_id) => FindId::Imdb(imdb_id.clone()),
            ExternalId::Tvdb(tvdb_id) => FindId::Tvdb(*tvdb_id),
            &ExternalId::Tmdb(tmdb_id) => {
                let movie = self.client.get_movie(tmdb_id).await;
                let movie = movie.map(|movie| movie.map(movie_details_record));
                log.push(KbQueryEvent::MovieDetails {
                    id: id.clone(),
                    found: movie
                        .as_ref()
                        .ok()
                        .and_then(Option::as_ref)
                        .map(movie_lookup),
                });
                return Ok(movie?);
            }
        };
        let found = self.client.find(find).await;
        let movie = found.map(|found| found.movie_results.first().map(movie_summary_record));
        log.push(KbQueryEvent::Find {
            id: id.clone(),
            found: movie
                .as_ref()
                .ok()
                .and_then(Option::as_ref)
                .map(movie_lookup),
        });
        Ok(movie?)
    }

    /// A TMDB ID's series, or the first that TMDB finds by another's ID.
    async fn series(
        &self,
        id: &ExternalId,
        log: &mut Vec<KbQueryEvent>,
    ) -> Result<Option<SeriesRecord>, KnowledgeBaseError> {
        let find = match id {
            ExternalId::Imdb(imdb_id) => FindId::Imdb(imdb_id.clone()),
            ExternalId::Tvdb(tvdb_id) => FindId::Tvdb(*tvdb_id),
            &ExternalId::Tmdb(tmdb_id) => {
                let tv = self.client.get_tv_series(tmdb_id).await;
                let tv = tv.map(|tv| tv.map(tv_details_record));
                log.push(KbQueryEvent::TvSeriesDetails {
                    id: id.clone(),
                    found: tv.as_ref().ok().and_then(Option::as_ref).map(series_lookup),
                });
                return Ok(tv?);
            }
        };
        let found = self.client.find(find).await;
        let tv = found.map(|found| found.tv_results.first().map(tv_summary_record));
        log.push(KbQueryEvent::Find {
            id: id.clone(),
            found: tv.as_ref().ok().and_then(Option::as_ref).map(series_lookup),
        });
        Ok(tv?)
    }

    async fn season(
        &self,
        series: &ExternalId,
        number: u32,
        log: &mut Vec<KbQueryEvent>,
    ) -> Result<Option<Season>, KnowledgeBaseError> {
        let &ExternalId::Tmdb(tmdb_id) = series else {
            return Err(anyhow!("not a TMDB ID: {series:?}").into());
        };
        log.push(KbQueryEvent::TvSeasonDetails {
            id: series.clone(),
            season: number,
        });
        let season = self.client.get_tv_season(tmdb_id, number).await?;
        Ok(season.map(season_record))
    }

    async fn movie_alternative_titles(
        &self,
        id: &ExternalId,
        log: &mut Vec<KbQueryEvent>,
    ) -> Result<Vec<String>, KnowledgeBaseError> {
        let &ExternalId::Tmdb(tmdb_id) = id else {
            return Ok(Vec::new());
        };
        log.push(KbQueryEvent::AlternativeTitles { id: id.clone() });
        Ok(self.client.get_movie_alternative_titles(tmdb_id).await?)
    }

    async fn series_alternative_titles(
        &self,
        id: &ExternalId,
        log: &mut Vec<KbQueryEvent>,
    ) -> Result<Vec<String>, KnowledgeBaseError> {
        let &ExternalId::Tmdb(tmdb_id) = id else {
            return Ok(Vec::new());
        };
        log.push(KbQueryEvent::AlternativeTitles { id: id.clone() });
        Ok(self.client.get_tv_alternative_titles(tmdb_id).await?)
    }

    async fn search_movies(
        &self,
        query: &str,
        year: Option<u32>,
    ) -> Result<Vec<MovieRecord>, KnowledgeBaseError> {
        let response = self.client.search_movies(query, year).await?;
        Ok(response.results.iter().map(movie_summary_record).collect())
    }

    async fn search_series(
        &self,
        query: &str,
        year: Option<u32>,
    ) -> Result<Vec<SeriesRecord>, KnowledgeBaseError> {
        let response = self.client.search_tv_series(query, year).await?;
        Ok(response.results.iter().map(tv_summary_record).collect())
    }

    async fn get_tv_series_details(
        &self,
        id: u32,
    ) -> Result<Option<TvSeriesKbData>, KnowledgeBaseError> {
        let Some(tv) = self.client.get_tv_series(id).await? else {
            return Ok(None);
        };
        let TvDetails {
            original_name,
            original_language,
            overview,
            tagline,
            first_air_date,
            poster_path,
            ..
        } = tv;
        Ok(Some(TvSeriesKbData {
            original_name: Some(original_name),
            original_language: non_empty(original_language),
            tagline: tagline.and_then(non_empty),
            overview: non_empty(overview),
            first_air_date: first_air_date.as_deref().and_then(parse_date),
            poster_path,
        }))
    }

    async fn get_movie_details(&self, id: u32) -> Result<Option<MovieKbData>, KnowledgeBaseError> {
        let Some(movie) = self.client.get_movie(id).await? else {
            return Ok(None);
        };
        let MovieDetails {
            original_title,
            original_language,
            overview,
            tagline,
            release_date,
            poster_path,
            runtime,
            ..
        } = movie;
        Ok(Some(MovieKbData {
            original_title: Some(original_title),
            original_language: non_empty(original_language),
            tagline: tagline.and_then(non_empty),
            overview: non_empty(overview),
            release_date: release_date.as_deref().and_then(parse_date),
            poster_path,
            runtime: runtime.map(|minutes| Duration::from_secs(u64::from(minutes) * 60)),
        }))
    }
}

fn movie_details_record(movie: MovieDetails) -> MovieRecord {
    MovieRecord {
        id: ExternalId::Tmdb(movie.id),
        release_year: movie.release_date.as_deref().and_then(year_of),
        title: movie.title,
        original_title: movie.original_title,
        runtime_minutes: movie.runtime,
        imdb_id: movie.imdb_id,
    }
}

/// A search result's movie, which TMDB doesn't give the runtime or IMDb ID
/// of.
fn movie_summary_record(movie: &MovieSummary) -> MovieRecord {
    MovieRecord {
        id: ExternalId::Tmdb(movie.id),
        title: movie.title.clone(),
        original_title: movie.original_title.clone(),
        release_year: movie.release_date.as_deref().and_then(year_of),
        runtime_minutes: None,
        imdb_id: None,
    }
}

/// What a lookup of a movie found, for its audit log.
fn movie_lookup(movie: &MovieRecord) -> KbLookupResult {
    KbLookupResult {
        id: movie.id.clone(),
        title: movie.title.clone(),
        year: movie.release_year,
        runtime_minutes: movie.runtime_minutes,
    }
}

/// What a lookup of a series found, for its audit log.
fn series_lookup(series: &SeriesRecord) -> KbLookupResult {
    KbLookupResult {
        id: series.id.clone(),
        title: series.name.clone(),
        year: series.first_air_year,
        runtime_minutes: None,
    }
}

fn tv_details_record(tv: TvDetails) -> SeriesRecord {
    SeriesRecord {
        id: ExternalId::Tmdb(tv.id),
        first_air_year: tv.first_air_date.as_deref().and_then(year_of),
        name: tv.name,
        original_name: tv.original_name,
        seasons: tv
            .seasons
            .into_iter()
            .map(|season| SeasonSummary {
                number: season.season_number,
                name: season.name,
                episode_count: season.episode_count,
            })
            .collect(),
    }
}

/// A search result's series, which TMDB doesn't list the seasons of.
fn tv_summary_record(tv: &TvSummary) -> SeriesRecord {
    SeriesRecord {
        id: ExternalId::Tmdb(tv.id),
        name: tv.name.clone(),
        original_name: tv.original_name.clone(),
        first_air_year: tv.first_air_date.as_deref().and_then(year_of),
        seasons: Vec::new(),
    }
}

fn season_record(season: SeasonDetails) -> Season {
    Season {
        number: season.season_number,
        episodes: season
            .episodes
            .into_iter()
            .map(|episode| Episode {
                number: episode.episode_number,
                name: episode.name,
                runtime_minutes: episode.runtime,
                production_code: episode
                    .production_code
                    .filter(|code| !code.trim().is_empty()),
                aired: episode.air_date.as_deref().and_then(parse_date),
            })
            .collect(),
    }
}

/// TMDB gives text it doesn't have, like a tagline, as an empty string.
fn non_empty(text: String) -> Option<String> {
    (!text.is_empty()).then_some(text)
}

fn parse_date(date: &str) -> Option<Date> {
    let mut parts = date.splitn(3, "-");
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    Some(Date { year, month, day })
}

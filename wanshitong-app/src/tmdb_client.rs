use crate::expiring_cache::ExpiringCache;
use crate::http_response_ext::ResponseExt;
use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
use reqwest::{Response, StatusCode};
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::policies::ExponentialBackoff;
use serde::Deserialize;
use std::{path::PathBuf, time::Duration};
use tokio::{sync::Mutex, time::Instant};

#[derive(Clone)]
pub struct TmdbClient {
    http_client: ClientWithMiddleware,
    token: String,
}

impl TmdbClient {
    /// A client that caches TMDB's responses in `cache_dir`.
    pub fn new(cache_dir: PathBuf, token: String) -> Self {
        Self::build(cache_dir, token, None)
    }

    /// A client that caches TMDB's responses in `cache_dir`, and sends at
    /// most one request each `interval` to TMDB, e.g. for a corpus searched
    /// in bulk. Answers from the cache aren't held back.
    pub fn with_rate_limit(cache_dir: PathBuf, token: String, interval: Duration) -> Self {
        Self::build(cache_dir, token, Some(interval))
    }

    fn build(cache_dir: PathBuf, token: String, interval: Option<Duration>) -> Self {
        let mut builder =
            reqwest_middleware::ClientBuilder::new(reqwest::ClientBuilder::new().build().unwrap())
                .with(reqwest_retry::RetryTransientMiddleware::new_with_policy(
                    ExponentialBackoff::builder().build_with_max_retries(10),
                ))
                .with(Cache(HttpCache {
                    mode: CacheMode::ForceCache,
                    // Deleting fully, so an expired response's contents go too.
                    manager: ExpiringCache(CACacheManager::new(cache_dir, true)),
                    options: HttpCacheOptions::default(),
                }));
        // After the cache, so that only what it can't answer waits.
        if let Some(interval) = interval {
            builder = builder.with(RateLimit {
                interval,
                next: Mutex::new(Instant::now()),
            });
        }
        let http_client = builder.build();

        Self { http_client, token }
    }

    async fn get(&self, url: &str) -> anyhow::Result<Response> {
        Ok(self
            .http_client
            .get(url)
            .bearer_auth(&self.token)
            .send()
            .await?
            .check_error()
            .await?)
    }

    async fn get_optional(&self, url: &str) -> anyhow::Result<Option<Response>> {
        let result = self
            .http_client
            .get(url)
            .bearer_auth(&self.token)
            .send()
            .await?
            .check_error()
            .await;
        match result {
            Ok(response) => Ok(Some(response)),
            Err(err) => {
                if err.status == StatusCode::NOT_FOUND {
                    Ok(None)
                } else {
                    Err(err.into())
                }
            }
        }
    }

    pub async fn find(&self, id: FindId) -> anyhow::Result<FindResponse> {
        Ok(self.get(&find_url(&id)).await?.json().await?)
    }

    pub async fn get_movie(&self, id: u32) -> anyhow::Result<Option<MovieDetails>> {
        let url = format!("https://api.themoviedb.org/3/movie/{id}");
        if let Some(response) = self.get_optional(&url).await? {
            Ok(response.json().await?)
        } else {
            Ok(None)
        }
    }
    pub async fn get_tv_series(&self, id: u32) -> anyhow::Result<Option<TvDetails>> {
        let url = format!("https://api.themoviedb.org/3/tv/{id}");
        if let Some(response) = self.get_optional(&url).await? {
            Ok(response.json().await?)
        } else {
            Ok(None)
        }
    }

    /// A season of a series, with its episodes, e.g. season 0, its specials.
    pub async fn get_tv_season(
        &self,
        id: u32,
        season: u32,
    ) -> anyhow::Result<Option<SeasonDetails>> {
        let url = format!("https://api.themoviedb.org/3/tv/{id}/season/{season}");
        if let Some(response) = self.get_optional(&url).await? {
            Ok(response.json().await?)
        } else {
            Ok(None)
        }
    }

    /// A movie's other titles, e.g. in other languages or romanized.
    pub async fn get_movie_alternative_titles(&self, id: u32) -> anyhow::Result<Vec<String>> {
        let url = format!("https://api.themoviedb.org/3/movie/{id}/alternative_titles");
        let Some(response) = self.get_optional(&url).await? else {
            return Ok(Vec::new());
        };
        let MovieAlternativeTitles { titles } = response.json().await?;
        Ok(titles.into_iter().map(|title| title.title).collect())
    }

    /// A series' other names, e.g. in other languages or romanized.
    pub async fn get_tv_alternative_titles(&self, id: u32) -> anyhow::Result<Vec<String>> {
        let url = format!("https://api.themoviedb.org/3/tv/{id}/alternative_titles");
        let Some(response) = self.get_optional(&url).await? else {
            return Ok(Vec::new());
        };
        let TvAlternativeTitles { results } = response.json().await?;
        Ok(results.into_iter().map(|title| title.title).collect())
    }

    pub async fn search_movies(
        &self,
        query: &str,
        year: Option<u32>,
    ) -> anyhow::Result<SearchMoviesResponse> {
        let url = search_url("movie", query, year);
        Ok(self.get(url.as_str()).await?.json().await?)
    }
    /// Series named like `query`, and first aired in `year`, if given.
    pub async fn search_tv_series(
        &self,
        query: &str,
        year: Option<u32>,
    ) -> anyhow::Result<SearchTvResponse> {
        let url = search_url("tv", query, year);
        Ok(self.get(url.as_str()).await?.json().await?)
    }
}

/// The URL of a search for `kind`, e.g. `movie` or `tv`, with its parameters
/// encoded, so that a query like `Tom & Jerry` is searched for whole. A series'
/// year is the one it first aired, as `year` would match any of its episodes'.
fn search_url(kind: &str, query: &str, year: Option<u32>) -> reqwest::Url {
    let mut params = vec![("query", query.to_string())];
    if let Some(year) = year {
        let name = if kind == "tv" {
            "first_air_date_year"
        } else {
            "year"
        };
        params.push((name, year.to_string()));
    }
    reqwest::Url::parse_with_params(
        &format!("https://api.themoviedb.org/3/search/{kind}"),
        &params,
    )
    .expect("TMDB search URLs are valid")
}

/// A movie's alternative titles, as TMDB lists them.
#[derive(Deserialize)]
struct MovieAlternativeTitles {
    #[serde(default)]
    titles: Vec<AlternativeTitle>,
}

/// A series' alternative titles, as TMDB lists them: as a movie's, but under
/// another key.
#[derive(Deserialize)]
struct TvAlternativeTitles {
    #[serde(default)]
    results: Vec<AlternativeTitle>,
}

#[derive(Deserialize)]
struct AlternativeTitle {
    title: String,
}

#[derive(Debug)]
pub enum FindId {
    /// E.g. `tt0113277`.
    Imdb(String),
    Tvdb(u32),
}

/// The URL of TMDB's find-by-ID endpoint for an IMDb or TVDB ID. The URL
/// carries the ID (an IMDb ID with a single `tt` prefix) and the source's
/// name, e.g. `imdb_id`. See
/// <https://developer.themoviedb.org/reference/find-by-id>.
fn find_url(id: &FindId) -> String {
    match id {
        FindId::Imdb(id) => format!(
            "https://api.themoviedb.org/3/find/tt{}?external_source=imdb_id",
            id.trim_start_matches("tt")
        ),
        FindId::Tvdb(id) => {
            format!("https://api.themoviedb.org/3/find/{id}?external_source=tvdb_id")
        }
    }
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct MovieDetails {
    pub id: u32,
    pub imdb_id: Option<String>,

    pub title: String,
    pub original_title: String,
    pub original_language: String,
    pub overview: String,
    pub tagline: Option<String>,

    pub release_date: Option<String>,
    /// Minutes. TMDB reports an unknown runtime as 0, which we map to `None`.
    #[serde(default, deserialize_with = "deserialize_runtime")]
    pub runtime: Option<u32>,

    pub adult: bool,
    // pub video: bool,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,

    pub status: Option<String>,

    pub budget: u64,
    pub revenue: u64,

    // pub popularity: f64,
    pub vote_average: f64,
    pub vote_count: u32,

    pub genres: Vec<Genre>,
    pub production_companies: Vec<ProductionCompany>,
    pub production_countries: Vec<ProductionCountry>,
    pub spoken_languages: Vec<SpokenLanguage>,
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct TvDetails {
    pub id: u32,

    pub name: String,
    pub original_name: String,
    pub original_language: String,
    pub overview: String,
    pub tagline: Option<String>,

    pub first_air_date: Option<String>,
    pub last_air_date: Option<String>,

    pub number_of_seasons: u32,
    pub number_of_episodes: u32,
    /// In order of their numbers, with specials as season 0.
    #[serde(default)]
    pub seasons: Vec<SeasonSummary>,

    pub in_production: bool,
    pub status: Option<String>,

    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,

    // pub popularity: f64,
    pub vote_average: f64,
    pub vote_count: u32,

    pub genres: Vec<Genre>,
    pub origin_country: Vec<String>,
    pub spoken_languages: Vec<SpokenLanguage>,
    pub production_companies: Vec<ProductionCompany>,
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct SearchMoviesResponse {
    pub results: Vec<MovieSummary>,
    pub page: u32,
    pub total_pages: u32,
    pub total_results: u32,
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct SearchTvResponse {
    pub page: u32,
    pub results: Vec<TvSummary>,
    pub total_pages: u32,
    pub total_results: u32,
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct FindResponse {
    pub movie_results: Vec<MovieSummary>,
    pub tv_results: Vec<TvSummary>,
    pub person_results: Vec<PersonSummary>,
    pub tv_episode_results: Vec<EpisodeSummary>,
    pub tv_season_results: Vec<SeasonSummary>,
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct MovieSummary {
    pub id: u32,
    pub title: String,
    pub original_title: String,
    pub original_language: String,
    pub overview: String,

    pub release_date: Option<String>,
    // pub adult: bool,
    // pub video: bool,
    // pub poster_path: Option<String>,
    // pub backdrop_path: Option<String>,
    // pub genre_ids: Vec<u32>,
    // pub popularity: f64,
    // pub vote_average: f64,
    // pub vote_count: u32,
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct TvSummary {
    pub id: u32,
    pub name: String,
    pub original_name: String,
    pub original_language: String,
    pub overview: String,
    pub first_air_date: Option<String>,
    // pub poster_path: Option<String>,
    // pub backdrop_path: Option<String>,

    // pub genre_ids: Vec<u32>,
    // pub popularity: f64,
    // pub vote_average: f64,
    // pub vote_count: u32,

    // pub origin_country: Vec<String>,
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct PersonSummary {
    pub id: u32,
    pub name: String,

    pub adult: bool,
    // pub popularity: f64,
    pub profile_path: Option<String>,

    /// Heterogeneous (movies or TV); usually left opaque
    pub known_for: Option<Vec<serde_json::Value>>,
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct EpisodeSummary {
    pub id: u32,
    pub name: String,
    pub overview: String,

    pub episode_number: u32,
    pub season_number: u32,

    pub air_date: Option<String>,

    pub still_path: Option<String>,
    pub vote_average: f64,
    pub vote_count: u32,
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct SeasonSummary {
    pub id: u32,
    pub name: String,
    pub overview: String,

    pub season_number: u32,
    pub air_date: Option<String>,
    /// As many as TMDB lists, which can be 0 for a season it knows no
    /// episodes of yet.
    #[serde(default)]
    pub episode_count: u32,

    pub poster_path: Option<String>,
}

/// A season of a series, with its episodes.
#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct SeasonDetails {
    pub season_number: u32,
    #[serde(default)]
    pub episodes: Vec<SeasonEpisode>,
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct SeasonEpisode {
    pub episode_number: u32,
    pub name: String,
    pub air_date: Option<String>,
    /// The studio's code for the episode, e.g. `DR-19`, as written in some
    /// releases' file names. Empty when TMDB doesn't know it.
    #[serde(default)]
    pub production_code: Option<String>,
    /// In minutes, if TMDB knows it.
    #[serde(default, deserialize_with = "deserialize_runtime")]
    pub runtime: Option<u32>,
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct Genre {
    pub id: u32,
    pub name: String,
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct ProductionCompany {
    pub id: u32,
    pub name: String,
    pub logo_path: Option<String>,
    pub origin_country: Option<String>,
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct ProductionCountry {
    pub iso_3166_1: String,
    pub name: String,
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize)]
pub struct SpokenLanguage {
    pub iso_639_1: String,
    pub name: String,
    pub english_name: Option<String>,
}

fn deserialize_runtime<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let runtime = Option::<u32>::deserialize(deserializer)?;
    Ok(runtime.filter(|&minutes| minutes != 0))
}

/// The year of a date as TMDB writes them, e.g. 2001 for `2001-12-19`.
/// Spaces requests at least `interval` apart, in the order they come.
struct RateLimit {
    interval: Duration,
    /// When the next request may be sent.
    next: Mutex<Instant>,
}

#[async_trait::async_trait]
impl reqwest_middleware::Middleware for RateLimit {
    async fn handle(
        &self,
        req: reqwest::Request,
        extensions: &mut http::Extensions,
        next: reqwest_middleware::Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        let at = {
            let mut next = self.next.lock().await;
            let at = (*next).max(Instant::now());
            *next = at + self.interval;
            at
        };
        tokio::time::sleep_until(at).await;
        next.run(req, extensions).await
    }
}

pub(crate) fn year_of(date: &str) -> Option<u32> {
    date.split('-').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_by_an_id_of_the_source_tmdb_names() {
        assert_eq!(
            find_url(&FindId::Imdb("tt0113277".into())),
            "https://api.themoviedb.org/3/find/tt0113277?external_source=imdb_id"
        );
        assert_eq!(
            find_url(&FindId::Imdb("0113277".into())),
            "https://api.themoviedb.org/3/find/tt0113277?external_source=imdb_id"
        );
        assert_eq!(
            find_url(&FindId::Tvdb(81797)),
            "https://api.themoviedb.org/3/find/81797?external_source=tvdb_id"
        );
    }

    fn runtime_of(runtime_json: &str) -> Option<u32> {
        #[derive(Deserialize)]
        struct Runtime {
            #[serde(default, deserialize_with = "deserialize_runtime")]
            runtime: Option<u32>,
        }
        serde_json::from_str::<Runtime>(&format!("{{{runtime_json}}}"))
            .unwrap()
            .runtime
    }

    #[test]
    fn zero_runtime_is_unknown() {
        assert_eq!(runtime_of(r#""runtime": 0"#), None);
        assert_eq!(runtime_of(r#""runtime": null"#), None);
        assert_eq!(runtime_of(""), None);
        assert_eq!(runtime_of(r#""runtime": 102"#), Some(102));
    }

    #[test]
    fn searches_for_the_whole_query() {
        let query = "suzumiya haruhi-chan no yuuutsu & nyoroon churuya-san [#1?+]";
        let url = search_url("tv", query, Some(2009));

        let params: Vec<_> = url.query_pairs().collect();
        assert_eq!(
            params,
            vec![
                ("query".into(), query.into()),
                ("first_air_date_year".into(), "2009".into())
            ]
        );
        assert_eq!(url.path(), "/3/search/tv");
    }
}

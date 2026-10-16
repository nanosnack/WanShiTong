use super::ApiError;
use crate::WebContext;
use axum::{
    Json,
    extract::{
        Path, Query, State,
        rejection::{PathRejection, QueryRejection},
    },
};
use wanshitong_api::v1::{self, ErrorCode};
use wanshitong_app::queries::{self, KbMovie, KbSearchHit, KbSeason, KbTvSeries, KbWorkKind};

/// Search the knowledge base
#[utoipa::path(
    get,
    path = "/kb/search",
    tag = "Knowledge base",
    params(v1::KbSearchQuery),
    responses((status = OK, body = v1::KbSearchResults), ApiError),
)]
pub(super) async fn search(
    State(ctx): State<WebContext>,
    query: Result<Query<v1::KbSearchQuery>, QueryRejection>,
) -> Result<Json<v1::KbSearchResults>, ApiError> {
    let Query(v1::KbSearchQuery { kind, query, year }) = query?;
    let kind = match kind {
        v1::WorkKind::Movie => KbWorkKind::Movie,
        v1::WorkKind::TvSeries => KbWorkKind::TvSeries,
    };
    let hits = queries::search_knowledge_base(&ctx.app, kind, &query, year).await?;
    Ok(Json(v1::KbSearchResults {
        results: hits
            .into_iter()
            .map(
                |KbSearchHit {
                     tmdb_id,
                     title,
                     original_title,
                     year,
                     overview,
                 }| v1::KbSearchHit {
                    id: v1::ExternalId::Tmdb(tmdb_id),
                    title,
                    original_title,
                    year,
                    overview,
                },
            )
            .collect(),
    }))
}

/// Get a movie from the knowledge base
#[utoipa::path(
    get,
    path = "/kb/movies/{tmdb_id}",
    tag = "Knowledge base",
    params(("tmdb_id" = u32, Path, description = "The movie's TMDB ID.")),
    responses((status = OK, body = v1::KbMovie), ApiError),
)]
pub(super) async fn movie(
    State(ctx): State<WebContext>,
    tmdb_id: Result<Path<u32>, PathRejection>,
) -> Result<Json<v1::KbMovie>, ApiError> {
    let Path(tmdb_id) = tmdb_id?;
    let KbMovie {
        tmdb_id,
        imdb_id,
        title,
        original_title,
        original_language,
        year,
        runtime_minutes,
        overview,
    } = queries::get_kb_movie(&ctx.app, tmdb_id)
        .await?
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, format!("no TMDB movie {tmdb_id}")))?;
    let ids = [
        Some(v1::ExternalId::Tmdb(tmdb_id)),
        imdb_id.map(v1::ExternalId::Imdb),
    ];
    Ok(Json(v1::KbMovie {
        ids: ids.into_iter().flatten().collect(),
        title,
        original_title,
        original_language,
        year,
        runtime_minutes,
        overview,
    }))
}

/// Get a TV series from the knowledge base
#[utoipa::path(
    get,
    path = "/kb/tv-series/{tmdb_id}",
    tag = "Knowledge base",
    params(("tmdb_id" = u32, Path, description = "The series' TMDB ID.")),
    responses((status = OK, body = v1::KbTvSeries), ApiError),
)]
pub(super) async fn tv_series(
    State(ctx): State<WebContext>,
    tmdb_id: Result<Path<u32>, PathRejection>,
) -> Result<Json<v1::KbTvSeries>, ApiError> {
    let Path(tmdb_id) = tmdb_id?;
    let KbTvSeries {
        tmdb_id,
        name,
        original_name,
        original_language,
        first_air_year,
        last_air_year,
        overview,
        seasons,
    } = queries::get_kb_tv_series(&ctx.app, tmdb_id)
        .await?
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, format!("no TMDB series {tmdb_id}")))?;
    Ok(Json(v1::KbTvSeries {
        ids: vec![v1::ExternalId::Tmdb(tmdb_id)],
        title: name,
        original_title: original_name,
        original_language,
        first_air_year,
        last_air_year,
        overview,
        seasons: seasons
            .into_iter()
            .map(
                |KbSeason {
                     number,
                     episode_count,
                     air_year,
                 }| v1::KbSeason {
                    number,
                    episode_count,
                    air_year,
                },
            )
            .collect(),
    }))
}

use crate::interpret::knowledge_base::KnowledgeBaseError;
use crate::tmdb_client::{TvDetails, year_of};
use crate::{AppContext, AppError};

pub struct KbTvSeries {
    pub tmdb_id: u32,
    pub name: String,
    pub original_name: String,
    pub original_language: String,
    pub first_air_year: Option<u32>,
    pub last_air_year: Option<u32>,
    pub overview: String,
    /// With specials as season 0.
    pub seasons: Vec<KbSeason>,
}

pub struct KbSeason {
    pub number: u32,
    pub episode_count: u32,
    pub air_year: Option<u32>,
}

/// The series with a TMDB ID, or None if there's none.
pub async fn get_kb_tv_series(
    ctx: &AppContext,
    tmdb_id: u32,
) -> Result<Option<KbTvSeries>, AppError> {
    let series = ctx
        .tmdb()?
        .get_tv_series(tmdb_id)
        .await
        .map_err(KnowledgeBaseError::from)?;
    Ok(series.map(
        |TvDetails {
             id,
             name,
             original_name,
             original_language,
             overview,
             first_air_date,
             last_air_date,
             seasons,
             ..
         }| KbTvSeries {
            tmdb_id: id,
            name,
            original_name,
            original_language,
            first_air_year: first_air_date.as_deref().and_then(year_of),
            last_air_year: last_air_date.as_deref().and_then(year_of),
            overview,
            seasons: seasons
                .into_iter()
                .map(|season| KbSeason {
                    number: season.season_number,
                    episode_count: season.episode_count,
                    air_year: season.air_date.as_deref().and_then(year_of),
                })
                .collect(),
        },
    ))
}

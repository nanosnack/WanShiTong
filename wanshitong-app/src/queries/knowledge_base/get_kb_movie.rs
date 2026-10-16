use crate::interpret::knowledge_base::KnowledgeBaseError;
use crate::tmdb_client::{MovieDetails, year_of};
use crate::{AppContext, AppError};

pub struct KbMovie {
    pub tmdb_id: u32,
    pub imdb_id: Option<String>,
    pub title: String,
    pub original_title: String,
    pub original_language: String,
    pub year: Option<u32>,
    pub runtime_minutes: Option<u32>,
    pub overview: String,
}

/// The movie with a TMDB ID, or None if there's none.
pub async fn get_kb_movie(ctx: &AppContext, tmdb_id: u32) -> Result<Option<KbMovie>, AppError> {
    let movie = ctx
        .tmdb()?
        .get_movie(tmdb_id)
        .await
        .map_err(KnowledgeBaseError::from)?;
    Ok(movie.map(
        |MovieDetails {
             id,
             imdb_id,
             title,
             original_title,
             original_language,
             overview,
             release_date,
             runtime,
             ..
         }| KbMovie {
            tmdb_id: id,
            imdb_id,
            title,
            original_title,
            original_language,
            year: release_date.as_deref().and_then(year_of),
            runtime_minutes: runtime,
            overview,
        },
    ))
}

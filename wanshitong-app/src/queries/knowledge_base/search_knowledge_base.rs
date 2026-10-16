use crate::interpret::knowledge_base::KnowledgeBaseError;
use crate::tmdb_client::year_of;
use crate::{AppContext, AppError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KbWorkKind {
    Movie,
    TvSeries,
}

/// A work that a knowledge base search found.
pub struct KbSearchHit {
    pub tmdb_id: u32,
    pub title: String,
    pub original_title: String,
    /// When it was released, or a series first aired.
    pub year: Option<u32>,
    pub overview: String,
}

/// Movies or series named like `query`, and released or first aired in
/// `year` if given, in the knowledge base's order.
pub async fn search_knowledge_base(
    ctx: &AppContext,
    kind: KbWorkKind,
    query: &str,
    year: Option<u32>,
) -> Result<Vec<KbSearchHit>, AppError> {
    let tmdb = ctx.tmdb()?;
    let hits = match kind {
        KbWorkKind::Movie => tmdb
            .search_movies(query, year)
            .await
            .map_err(KnowledgeBaseError::from)?
            .results
            .into_iter()
            .map(|movie| KbSearchHit {
                tmdb_id: movie.id,
                title: movie.title,
                original_title: movie.original_title,
                year: movie.release_date.as_deref().and_then(year_of),
                overview: movie.overview,
            })
            .collect(),
        KbWorkKind::TvSeries => tmdb
            .search_tv_series(query, year)
            .await
            .map_err(KnowledgeBaseError::from)?
            .results
            .into_iter()
            .map(|series| KbSearchHit {
                tmdb_id: series.id,
                title: series.name,
                original_title: series.original_name,
                year: series.first_air_date.as_deref().and_then(year_of),
                overview: series.overview,
            })
            .collect(),
    };
    Ok(hits)
}

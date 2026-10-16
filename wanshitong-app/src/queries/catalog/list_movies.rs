use crate::{AppContext, AppError};
use wanshitong_core::queries::get_canonical_interpretation_id;
use wanshitong_core::{self as core};

pub use core::queries::{MovieCursor, MovieSearchQuery, MovieSummaryList};

pub async fn list_movies(
    ctx: &AppContext,
    query: &MovieSearchQuery,
) -> Result<MovieSummaryList, AppError> {
    let Some(canonical_interpretation_id) = get_canonical_interpretation_id(&ctx.core).await?
    else {
        return Ok(MovieSummaryList {
            movies: Vec::new(),
            next_cursor: None,
        });
    };
    let page = core::queries::list_movies(&ctx.core, canonical_interpretation_id, query).await?;
    Ok(page)
}

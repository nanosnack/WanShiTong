use crate::{AppContext, AppError};
use wanshitong_core::queries::get_canonical_interpretation_id;
use wanshitong_core::{self as core};

pub use core::queries::{TvSeriesCursor, TvSeriesSearchQuery, TvSeriesSummaryList};

pub async fn list_tv_series(
    ctx: &AppContext,
    query: &TvSeriesSearchQuery,
) -> Result<TvSeriesSummaryList, AppError> {
    let Some(canonical_interpretation_id) = get_canonical_interpretation_id(&ctx.core).await?
    else {
        return Ok(TvSeriesSummaryList {
            series: Vec::new(),
            next_cursor: None,
        });
    };
    let page = core::queries::list_tv_series(&ctx.core, canonical_interpretation_id, query).await?;
    Ok(page)
}

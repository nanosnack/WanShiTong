use crate::{AppContext, AppError};
use wanshitong_core as core;
use wanshitong_core::domain::{TvSeriesDetails, TvSeriesUuid};

pub async fn get_tv_series(
    ctx: &AppContext,
    uuid: TvSeriesUuid,
) -> Result<Option<TvSeriesDetails>, AppError> {
    Ok(core::queries::get_tv_series(&ctx.core, uuid).await?)
}

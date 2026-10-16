use crate::{AppContext, AppError};
use wanshitong_core as core;
use wanshitong_core::domain::{MovieDetails, MovieUuid};

pub async fn get_movie(
    ctx: &AppContext,
    uuid: MovieUuid,
) -> Result<Option<MovieDetails>, AppError> {
    Ok(core::queries::get_movie(&ctx.core, uuid).await?)
}

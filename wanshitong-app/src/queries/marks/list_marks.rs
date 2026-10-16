use crate::{AppContext, AppError};
use wanshitong_core::domain::Mark;
use wanshitong_core::queries as core;

/// Marks, newest first, with how far correcting each has got.
pub async fn list_marks(ctx: &AppContext) -> Result<Vec<Mark>, AppError> {
    Ok(core::list_marks(&ctx.core).await?)
}

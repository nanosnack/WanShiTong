use crate::{AppContext, AppError};
use wanshitong_core::commands as core;
use wanshitong_core::domain::MarkUuid;

/// Removes a mark.
pub async fn unmark_files(ctx: &AppContext, uuid: MarkUuid) -> Result<(), AppError> {
    Ok(core::delete_mark(&ctx.core, uuid).await?)
}

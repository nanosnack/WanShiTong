use crate::{AppContext, AppError};
use wanshitong_core::domain::InterpretationUuid;

pub async fn rename_interpretation(
    ctx: &AppContext,
    uuid: InterpretationUuid,
    name: &str,
) -> Result<(), AppError> {
    wanshitong_core::commands::rename_interpretation(&ctx.core, uuid, name).await?;
    Ok(())
}

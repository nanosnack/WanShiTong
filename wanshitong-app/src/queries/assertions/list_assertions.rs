use crate::{AppContext, AppError, AppErrorContext};
use wanshitong_core::domain::{Assertion, InterpretationUuid};

/// Assertions that new interpretations will include, newest first.
pub async fn list_active_assertions(ctx: &AppContext) -> Result<Vec<Assertion>, AppError> {
    Ok(wanshitong_core::queries::list_active_assertions(&ctx.core).await?)
}

/// The assertions an interpretation was derived from, retracted since or not.
pub async fn list_interpretation_assertions(
    ctx: &AppContext,
    uuid: InterpretationUuid,
) -> Result<Vec<Assertion>, AppError> {
    let interpretation = wanshitong_core::queries::get_interpretation(&ctx.core, uuid)
        .await?
        .ok_or(AppErrorContext::InterpretationNotFound(uuid))?;
    Ok(
        wanshitong_core::queries::list_interpretation_assertions(&ctx.core, interpretation.id)
            .await?,
    )
}

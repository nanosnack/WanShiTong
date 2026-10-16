use crate::interpretations::InterpretationStatus;
use crate::{AppContext, AppError, AppErrorContext};
use wanshitong_core::domain::InterpretationUuid;

pub async fn get_interpretation(
    ctx: &AppContext,
    uuid: InterpretationUuid,
) -> Result<Option<InterpretationStatus>, AppError> {
    let interpretation = wanshitong_core::queries::get_interpretation(&ctx.core, uuid).await?;
    Ok(interpretation.map(|interpretation| InterpretationStatus::new(ctx, interpretation)))
}

/// The interpretation with `uuid`, or the canonical one when there's no `uuid`.
pub async fn get_interpretation_or_canonical(
    ctx: &AppContext,
    uuid: Option<InterpretationUuid>,
) -> Result<InterpretationStatus, AppError> {
    let interpretation = match uuid {
        Some(uuid) => wanshitong_core::queries::get_interpretation(&ctx.core, uuid)
            .await?
            .ok_or(AppErrorContext::InterpretationNotFound(uuid))?,
        None => wanshitong_core::queries::get_canonical_interpretation(&ctx.core)
            .await?
            .ok_or(AppErrorContext::NoCatalog)?,
    };
    Ok(InterpretationStatus::new(ctx, interpretation))
}

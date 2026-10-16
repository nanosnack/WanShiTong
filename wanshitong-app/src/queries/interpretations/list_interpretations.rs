use crate::interpretations::InterpretationStatus;
use crate::{AppContext, AppError};
use wanshitong_core::domain::ChangesetUuid;

/// All interpretations, newest first.
pub async fn list_interpretations(ctx: &AppContext) -> Result<Vec<InterpretationStatus>, AppError> {
    let interpretations = wanshitong_core::queries::list_interpretations(&ctx.core).await?;
    Ok(interpretations
        .into_iter()
        .map(|interpretation| InterpretationStatus::new(ctx, interpretation))
        .collect())
}

/// A changeset's interpretations, newest first: those made to preview or to
/// accept it.
pub async fn list_changeset_interpretations(
    ctx: &AppContext,
    changeset: ChangesetUuid,
) -> Result<Vec<InterpretationStatus>, AppError> {
    let interpretations =
        wanshitong_core::queries::list_changeset_interpretations(&ctx.core, changeset).await?;
    Ok(interpretations
        .into_iter()
        .map(|interpretation| InterpretationStatus::new(ctx, interpretation))
        .collect())
}

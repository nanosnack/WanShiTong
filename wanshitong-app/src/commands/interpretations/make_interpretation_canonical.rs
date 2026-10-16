use crate::commands::refresh_auto_refresh_views;
use crate::interpretations::InterpretationState;
use crate::{AppContext, AppError, AppErrorContext};
use wanshitong_core::commands::set_canonical_interpretation;
use wanshitong_core::domain::{InterpretationRowId, InterpretationUuid};

/// Makes a successful interpretation's catalog the one shown and used by
/// views.
pub async fn make_interpretation_canonical(
    ctx: &AppContext,
    uuid: InterpretationUuid,
) -> Result<(), AppError> {
    let interpretation = crate::queries::get_interpretation(ctx, uuid)
        .await?
        .ok_or(AppErrorContext::InterpretationNotFound(uuid))?;
    if interpretation.state != InterpretationState::Succeeded {
        return Err(AppErrorContext::InterpretationNotSucceeded(uuid).into());
    }
    change_canonical_interpretation(ctx, interpretation.interpretation.id).await
}

/// Makes an interpretation canonical, then refreshes the views that keep up
/// with the catalog. The only way the canonical interpretation changes, so
/// that what follows from it can't be missed.
pub(crate) async fn change_canonical_interpretation(
    ctx: &AppContext,
    id: InterpretationRowId,
) -> Result<(), AppError> {
    // The one call allowed by wanshitong-app/clippy.toml.
    #[allow(clippy::disallowed_methods)]
    set_canonical_interpretation(&ctx.core, id).await?;
    refresh_auto_refresh_views(ctx).await;
    Ok(())
}

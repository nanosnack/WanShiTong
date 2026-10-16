use crate::commands::refresh_view;
use crate::views::ViewState;
use crate::{AppContext, AppError, AppErrorContext};
use wanshitong_core::domain::ViewUuid;
use wanshitong_core::queries::get_canonical_interpretation_id;

/// Sets whether a view is refreshed whenever another interpretation becomes
/// canonical. Turning it on also refreshes the view if it's behind the
/// catalog, since it would otherwise stay behind until the next change.
pub async fn set_view_auto_refresh(
    ctx: &AppContext,
    uuid: ViewUuid,
    auto_refresh: bool,
) -> Result<(), AppError> {
    wanshitong_core::commands::set_view_auto_refresh(&ctx.core, uuid, auto_refresh).await?;
    if !auto_refresh || get_canonical_interpretation_id(&ctx.core).await?.is_none() {
        return Ok(());
    }
    let status = crate::queries::get_view(ctx, uuid)
        .await?
        .ok_or(AppErrorContext::ViewNotFound(uuid))?;
    if status.state != ViewState::UpToDate {
        refresh_view(ctx, uuid).await?;
    }
    Ok(())
}

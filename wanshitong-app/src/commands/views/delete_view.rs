use crate::views::dematerialize;
use crate::{AppContext, AppError, AppErrorContext};
use tracing::info;
use wanshitong_core::domain::ViewUuid;
use wanshitong_core::queries::get_view;

/// Removes a view's symlinks, then the view itself.
///
/// If the files can't be removed (say, something other than the view's own
/// links is in the way), the view is kept, so deleting can be retried.
pub async fn delete_view(ctx: &AppContext, uuid: ViewUuid) -> Result<(), AppError> {
    // A refresh mid-way would recreate what's being removed.
    let _guard = ctx.view_refresh.lock().await;

    let view = get_view(&ctx.core, uuid)
        .await?
        .ok_or(AppErrorContext::ViewNotFound(uuid))?;
    let removed = dematerialize(&view.config).await?;
    wanshitong_core::commands::delete_view(&ctx.core, uuid).await?;
    info!("Deleted view \"{}\" and its {removed} links", view.name);
    Ok(())
}

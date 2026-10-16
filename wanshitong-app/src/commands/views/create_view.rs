use crate::commands::refresh_view;
use crate::report_error;
use crate::{AppContext, AppError};
use wanshitong_core::domain::{View, ViewConfig};
use wanshitong_core::queries::get_canonical_interpretation_id;

/// Creates a view, then refreshes it if there's a catalog to refresh it from.
/// If `auto_refresh`, it's refreshed again whenever another interpretation
/// becomes canonical.
///
/// A failed refresh is recorded as the view's state, as when refreshing it
/// later, so the view is created either way.
pub async fn create_view(
    ctx: &AppContext,
    name: impl AsRef<str>,
    config: ViewConfig,
    auto_refresh: bool,
) -> Result<View, AppError> {
    let view =
        wanshitong_core::commands::create_view(&ctx.core, name, config, auto_refresh).await?;
    if get_canonical_interpretation_id(&ctx.core).await?.is_none() {
        return Ok(view);
    }
    match refresh_view(ctx, view.uuid).await {
        Ok(status) => Ok(status.view),
        Err(e) => {
            report_error!(e, view = %view.name, "Failed to refresh new view");
            Ok(view)
        }
    }
}

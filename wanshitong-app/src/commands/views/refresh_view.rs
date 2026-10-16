use crate::report_error;
use crate::views::{ViewState, ViewStatus, materialize};
use crate::{AppContext, AppError, AppErrorContext};
use jiff::Timestamp;
use wanshitong_core::commands::{NewViewMaterialization, record_view_materialization};
use wanshitong_core::domain::ViewUuid;
use wanshitong_core::queries::{get_canonical_interpretation_id, get_view};

/// Brings a view up to date with the canonical catalog.
///
/// A failure to materialize is recorded as the view's state rather than
/// returned, since it's something to show about the view.
pub async fn refresh_view(ctx: &AppContext, uuid: ViewUuid) -> Result<ViewStatus, AppError> {
    // Two refreshes of one view would trip over each other's changes.
    let _guard = ctx.view_refresh.lock().await;

    let view = get_view(&ctx.core, uuid)
        .await?
        .ok_or(AppErrorContext::ViewNotFound(uuid))?;
    let started_at = Timestamp::now();
    let interpretation_id = get_canonical_interpretation_id(&ctx.core).await?;
    let result = match interpretation_id {
        Some(interpretation_id) => materialize(ctx, &view.config, interpretation_id).await,
        None => Err(AppErrorContext::NoCatalog.into()),
    };
    if let Err(e) = &result {
        report_error!(e, view = %view.name, "Failed to refresh view");
    }

    record_view_materialization(
        &ctx.core,
        NewViewMaterialization {
            view: uuid,
            interpretation_id,
            started_at,
            stopped_at: Timestamp::now(),
            result: result.map_err(|e| format!("{e:#}")),
        },
    )
    .await?;

    let view = get_view(&ctx.core, uuid)
        .await?
        .ok_or(AppErrorContext::ViewNotFound(uuid))?;
    Ok(ViewStatus::new(view, interpretation_id))
}

/// Refreshes every view set to keep up with the canonical catalog that isn't
/// up to date with it, after another interpretation has become canonical.
///
/// Failures are logged rather than returned, since the change of canonical
/// interpretation has already happened and each view records its own state.
pub(crate) async fn refresh_auto_refresh_views(ctx: &AppContext) {
    let views = match crate::queries::list_views(ctx).await {
        Ok(views) => views,
        Err(e) => {
            report_error!(e, "Failed to list views to refresh");
            return;
        }
    };
    for status in views {
        if !status.view.auto_refresh || status.state == ViewState::UpToDate {
            continue;
        }
        if let Err(e) = refresh_view(ctx, status.view.uuid).await {
            report_error!(e, view = %status.view.name, "Failed to refresh view");
        }
    }
}

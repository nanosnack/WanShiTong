use crate::commands::{create_interpretation, fit_name};
use crate::queries::get_data_source;
use crate::{AppContext, AppError, CoreErrorContext};
use wanshitong_core::domain::{DataSourceUuid, InterpretationUuid};

/// Deletes a data source, then starts an interpretation without it, to become
/// canonical once it succeeds. Until then, the catalog and views still include
/// it.
pub async fn delete_data_source(
    ctx: &AppContext,
    uuid: DataSourceUuid,
) -> Result<InterpretationUuid, AppError> {
    let source = get_data_source(ctx, uuid)
        .await?
        .ok_or(CoreErrorContext::DataSourceNotFound(uuid))?;
    wanshitong_core::commands::delete_data_source(&ctx.core, uuid).await?;
    ctx.data_sources_changed.notify_one();
    create_interpretation(ctx, &fit_name(format!("Deleted {}", source.name)), true).await
}

/// Restores a deleted data source, then starts an interpretation with it
/// again, to become canonical once it succeeds.
pub async fn restore_data_source(
    ctx: &AppContext,
    uuid: DataSourceUuid,
) -> Result<InterpretationUuid, AppError> {
    let source = get_data_source(ctx, uuid)
        .await?
        .ok_or(CoreErrorContext::DataSourceNotFound(uuid))?;
    wanshitong_core::commands::restore_data_source(&ctx.core, uuid).await?;
    ctx.data_sources_changed.notify_one();
    create_interpretation(ctx, &fit_name(format!("Restored {}", source.name)), true).await
}

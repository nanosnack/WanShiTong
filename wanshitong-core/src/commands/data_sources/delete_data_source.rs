use super::conflicts::{check_no_view_overlap, name_error};
use crate::{CoreContext, CoreError, CoreErrorContext, domain::DataSourceUuid, time::unix_now_i64};
use std::path::Path;

/// Tombstones a data source: it takes no part in new ingests, assertions or
/// interpretations. Past ones, including the canonical interpretation, still
/// refer to it until they're replaced. Deleting a deleted source does nothing.
pub async fn delete_data_source(ctx: &CoreContext, uuid: DataSourceUuid) -> Result<(), CoreError> {
    let now = unix_now_i64();
    let updated = sqlx::query!(
        "UPDATE data_sources SET deleted_at = COALESCE(deleted_at, ?) WHERE uuid = ?",
        now,
        uuid,
    )
    .execute(&ctx.writer)
    .await
    .map_err(CoreErrorContext::from)?;
    if updated.rows_affected() == 0 {
        error_stack::bail!(CoreErrorContext::DataSourceNotFound(uuid));
    }
    Ok(())
}

/// Undoes [`delete_data_source`], with the source's past ingests and
/// assertions. Fails if another data source took its name, or a view took its
/// place, in the meantime. Restoring an undeleted source does nothing.
pub async fn restore_data_source(ctx: &CoreContext, uuid: DataSourceUuid) -> Result<(), CoreError> {
    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;
    let source = sqlx::query!(
        r#"
        SELECT ds.name, ds.deleted_at, fs.root_path
        FROM data_sources ds
        JOIN fs_data_sources fs
            ON fs.id = ds.id
        WHERE ds.uuid = ?
        "#,
        uuid
    )
    .fetch_optional(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?
    .ok_or(CoreErrorContext::DataSourceNotFound(uuid))?;
    if source.deleted_at.is_none() {
        return Ok(());
    }
    check_no_view_overlap(&mut txn, Path::new(&source.root_path)).await?;
    sqlx::query!(
        "UPDATE data_sources SET deleted_at = NULL WHERE uuid = ?",
        uuid
    )
    .execute(&mut *txn)
    .await
    .map_err(|e| name_error(e, source.name))?;
    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(())
}

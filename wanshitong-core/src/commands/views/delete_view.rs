use crate::{CoreContext, CoreError, CoreErrorContext, domain::ViewUuid};

/// Deletes a view and its materialization history. Its files are left alone;
/// removing them is up to the caller.
pub async fn delete_view(ctx: &CoreContext, uuid: ViewUuid) -> Result<(), CoreError> {
    let deleted = sqlx::query!("DELETE FROM views WHERE uuid = ?", uuid)
        .execute(&ctx.writer)
        .await
        .map_err(CoreErrorContext::from)?;
    if deleted.rows_affected() == 0 {
        error_stack::bail!(CoreErrorContext::ViewNotFound(uuid));
    }
    Ok(())
}

use crate::{CoreContext, CoreError, CoreErrorContext, domain::ViewUuid};

/// Sets whether a view is refreshed whenever another interpretation becomes
/// canonical.
pub async fn set_view_auto_refresh(
    ctx: &CoreContext,
    uuid: ViewUuid,
    auto_refresh: bool,
) -> Result<(), CoreError> {
    let updated = sqlx::query!(
        "UPDATE views SET auto_refresh = ? WHERE uuid = ?",
        auto_refresh,
        uuid
    )
    .execute(&ctx.writer)
    .await
    .map_err(CoreErrorContext::from)?;
    if updated.rows_affected() == 0 {
        error_stack::bail!(CoreErrorContext::ViewNotFound(uuid));
    }
    Ok(())
}

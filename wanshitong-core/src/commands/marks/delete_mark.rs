use crate::{CoreContext, CoreError, CoreErrorContext, domain::MarkUuid};

/// Removes a mark. Removing it again changes nothing.
pub async fn delete_mark(ctx: &CoreContext, uuid: MarkUuid) -> Result<(), CoreError> {
    sqlx::query!("DELETE FROM marks WHERE uuid = ?", uuid)
        .execute(&ctx.writer)
        .await
        .map_err(CoreErrorContext::from)?;
    Ok(())
}

use crate::{
    CoreContext, CoreError, CoreErrorContext, changesets::find_open_changeset,
    domain::ChangesetUuid, time::unix_now_i64,
};

/// Drops an open changeset's pending changes. The changeset stays, as
/// history, and no interpretation but its own ever includes them.
pub async fn discard_changeset(ctx: &CoreContext, uuid: ChangesetUuid) -> Result<(), CoreError> {
    let now = unix_now_i64();
    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;
    let changeset_id = find_open_changeset(&mut txn, uuid).await?;
    sqlx::query!(
        "UPDATE changesets SET state = 'discarded', closed_at = ? WHERE id = ?",
        now,
        changeset_id,
    )
    .execute(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;
    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(())
}

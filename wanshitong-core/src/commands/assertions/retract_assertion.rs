use crate::domain::ChangesetState;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    changesets::find_open_changeset,
    domain::{AssertionUuid, ChangesetRowId, ChangesetUuid},
    time::unix_now_i64,
};

/// Retracts an assertion in an open changeset.
///
/// Retracting an assertion that was made in this changeset just drops it.
/// Retracting an assertion in a separate accepted changeset will take effect
/// once this changeset is accepted. Retracting either again changes nothing.
pub async fn retract_assertion(
    ctx: &CoreContext,
    changeset: ChangesetUuid,
    assertion: AssertionUuid,
) -> Result<(), CoreError> {
    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;
    let changeset_id = find_open_changeset(&mut txn, changeset).await?;
    let group = sqlx::query!(
        r#"
        SELECT
            g.id AS "id!",
            g.changeset_id AS "changeset_id: ChangesetRowId",
            c.state,
            EXISTS (
                SELECT 1 FROM assertions a
                WHERE a.group_id = g.id AND a.retracted_at IS NULL
            ) AS "in_effect!: bool"
        FROM assertion_groups g
        JOIN changesets c
            ON c.id = g.changeset_id
        WHERE g.uuid = ?
        "#,
        assertion,
    )
    .fetch_optional(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?
    .ok_or(CoreErrorContext::AssertionNotFound(assertion))?;

    if group.changeset_id == changeset_id {
        let now = unix_now_i64();
        sqlx::query!(
            "UPDATE assertions SET retracted_at = ? WHERE group_id = ? AND retracted_at IS NULL",
            now,
            group.id,
        )
        .execute(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?;
    } else if ChangesetState::from_db_str(&group.state)? == ChangesetState::Accepted
        && group.in_effect
    {
        sqlx::query!(
            r#"
            INSERT INTO changeset_retractions (changeset_id, assertion_group_id)
            VALUES (?, ?)
            ON CONFLICT DO NOTHING
            "#,
            changeset_id,
            group.id,
        )
        .execute(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?;
    } else {
        error_stack::bail!(CoreErrorContext::AssertionNotRetractable {
            changeset,
            assertion,
        });
    }
    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(())
}

/// Takes back an open changeset's retraction of an accepted assertion, so
/// that accepting it leaves the assertion in effect. Taking back one it
/// doesn't make changes nothing.
pub async fn undo_retraction(
    ctx: &CoreContext,
    changeset: ChangesetUuid,
    assertion: AssertionUuid,
) -> Result<(), CoreError> {
    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;
    let changeset_id = find_open_changeset(&mut txn, changeset).await?;
    sqlx::query!(
        r#"
        DELETE FROM changeset_retractions
        WHERE
            changeset_id = ?
            AND assertion_group_id IN (SELECT id FROM assertion_groups WHERE uuid = ?)
        "#,
        changeset_id,
        assertion,
    )
    .execute(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;
    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(())
}

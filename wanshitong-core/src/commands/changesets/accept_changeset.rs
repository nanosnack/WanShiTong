use crate::{
    CoreContext, CoreError, CoreErrorContext,
    changesets::find_open_changeset,
    domain::{ChangesetUuid, ClaimPredicate},
    json,
    time::unix_now_i64,
};

/// Applies an open changeset's changes: its assertions become active,
/// replacing any older ones of the same kind about the same files, and the
/// assertions it retracts are retracted. New interpretations include them.
pub async fn accept_changeset(ctx: &CoreContext, uuid: ChangesetUuid) -> Result<(), CoreError> {
    let now = unix_now_i64();
    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;
    let changeset_id = find_open_changeset(&mut txn, uuid).await?;

    let pending = sqlx::query!(
        r#"
        SELECT a.data_source_id AS "data_source_id!", a.rel_path, a.claim
        FROM assertions a
        JOIN assertion_groups g
            ON g.id = a.group_id
        WHERE g.changeset_id = ? AND a.retracted_at IS NULL
        "#,
        changeset_id,
    )
    .fetch_all(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;
    for row in pending {
        let kind = json::decode::<ClaimPredicate>(&row.claim)?.kind();
        let active = sqlx::query!(
            r#"
            SELECT a.id AS "id!", a.claim
            FROM assertions a
            JOIN assertion_groups g
                ON g.id = a.group_id
            JOIN changesets c
                ON c.id = g.changeset_id
            WHERE
                a.data_source_id = ?
                AND a.rel_path = ?
                AND a.retracted_at IS NULL
                AND c.state = 'accepted'
            "#,
            row.data_source_id,
            row.rel_path,
        )
        .fetch_all(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?;
        for older in active {
            if json::decode::<ClaimPredicate>(&older.claim)?.kind() == kind {
                sqlx::query!(
                    "UPDATE assertions SET retracted_at = ? WHERE id = ?",
                    now,
                    older.id,
                )
                .execute(&mut *txn)
                .await
                .map_err(CoreErrorContext::from)?;
            }
        }
    }

    sqlx::query!(
        r#"
        UPDATE assertions
        SET retracted_at = ?
        WHERE
            retracted_at IS NULL
            AND group_id IN (
                SELECT assertion_group_id
                FROM changeset_retractions
                WHERE changeset_id = ?
            )
        "#,
        now,
        changeset_id,
    )
    .execute(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;
    sqlx::query!(
        "UPDATE changesets SET state = 'accepted', closed_at = ? WHERE id = ?",
        now,
        changeset_id,
    )
    .execute(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;
    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(())
}

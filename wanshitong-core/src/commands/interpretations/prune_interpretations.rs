use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{InterpretationRowId, InterpretationUuid},
};
use jiff::Timestamp;

/// Deletes the interpretations started before `before`, with their catalogs,
/// but for the canonical one, any still running, and an open changeset's,
/// whose latest is its preview and what accepting it reuses. Returns the
/// UUIDs of those deleted, oldest first.
///
/// Every interpretation keeps a whole catalog, so without this the database
/// grows with each correction and never shrinks. Each is deleted in a
/// transaction of its own, so that a long prune doesn't hold other writers
/// up.
pub async fn prune_interpretations(
    ctx: &CoreContext,
    before: Timestamp,
) -> Result<Vec<InterpretationUuid>, CoreError> {
    let before = before.as_second();
    let candidates = sqlx::query!(
        r#"
        SELECT
            i.id AS "id!: InterpretationRowId",
            i.uuid AS "uuid!: InterpretationUuid"
        FROM interpretations i
        JOIN system_state ss
        LEFT JOIN changesets c
            ON c.id = i.changeset_id
        WHERE
            i.stopped_at IS NOT NULL
            AND i.started_at < ?
            AND i.id IS NOT ss.canonical_interpretation_id
            AND (c.state IS NULL OR c.state <> 'open')
        ORDER BY i.id
        "#,
        before,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;

    let mut deleted = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        // It may have become canonical since it was selected.
        let result = sqlx::query!(
            r#"
            DELETE FROM interpretations
            WHERE
                id = ?
                AND id IS NOT (SELECT canonical_interpretation_id FROM system_state)
            "#,
            candidate.id,
        )
        .execute(&ctx.writer)
        .await
        .map_err(CoreErrorContext::from)?;
        if result.rows_affected() == 1 {
            deleted.push(candidate.uuid);
        }
    }
    Ok(deleted)
}

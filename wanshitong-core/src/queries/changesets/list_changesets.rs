use crate::time::stored_timestamp;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{ChangesetState, ChangesetUuid},
};
use jiff::Timestamp;

/// A changeset in brief, for listing.
#[derive(Clone, Debug)]
pub struct ChangesetSummary {
    pub uuid: ChangesetUuid,
    pub author: String,
    pub description: String,
    pub state: ChangesetState,
    pub created_at: Timestamp,
    pub closed_at: Option<Timestamp>,
    /// Not counting those withdrawn while it was open.
    pub assertion_count: u64,
    pub retraction_count: u64,
}

/// Changesets, newest first, only those in `state` if given.
pub async fn list_changesets(
    ctx: &CoreContext,
    state: Option<ChangesetState>,
) -> Result<Vec<ChangesetSummary>, CoreError> {
    let state = state.map(ChangesetState::as_db_str);
    let rows = sqlx::query!(
        r#"
        SELECT
            c.uuid AS "uuid!: ChangesetUuid",
            c.author,
            c.description,
            c.state,
            c.created_at,
            c.closed_at,
            (
                SELECT COUNT(*)
                FROM assertion_groups g
                WHERE g.changeset_id = c.id AND EXISTS (
                    SELECT 1 FROM assertions a
                    WHERE a.group_id = g.id
                        AND (a.retracted_at IS NULL OR c.state != 'open')
                )
            ) AS "assertion_count!: i64",
            (
                SELECT COUNT(*) FROM changeset_retractions r WHERE r.changeset_id = c.id
            ) AS "retraction_count!: i64"
        FROM changesets c
        WHERE ?1 IS NULL OR c.state = ?1
        ORDER BY c.created_at DESC, c.id DESC
        "#,
        state,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;
    rows.into_iter()
        .map(|row| {
            Ok(ChangesetSummary {
                uuid: row.uuid,
                author: row.author,
                description: row.description,
                state: ChangesetState::from_db_str(&row.state)?,
                created_at: stored_timestamp(row.created_at),
                closed_at: row.closed_at.map(stored_timestamp),
                assertion_count: row.assertion_count as u64,
                retraction_count: row.retraction_count as u64,
            })
        })
        .collect()
}

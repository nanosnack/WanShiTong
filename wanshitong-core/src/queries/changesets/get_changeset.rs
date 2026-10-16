use crate::time::stored_timestamp;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{AssertionUuid, ChangesetDetails, ChangesetRowId, ChangesetState, ChangesetUuid},
    queries::list_changeset_assertions,
};

/// A changeset, with its assertions and retractions.
pub async fn get_changeset(
    ctx: &CoreContext,
    uuid: ChangesetUuid,
) -> Result<Option<ChangesetDetails>, CoreError> {
    let mut conn = ctx.reader.acquire().await.map_err(CoreErrorContext::from)?;
    let Some(row) = sqlx::query!(
        r#"
        SELECT
            id AS "id!: ChangesetRowId",
            author,
            description,
            state,
            created_at,
            closed_at
        FROM changesets
        WHERE uuid = ?
        "#,
        uuid,
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(CoreErrorContext::from)?
    else {
        return Ok(None);
    };
    let assertions = list_changeset_assertions(&mut conn, row.id).await?;
    let retractions = sqlx::query_scalar!(
        r#"
        SELECT g.uuid AS "uuid!: AssertionUuid"
        FROM changeset_retractions r
        JOIN assertion_groups g
            ON g.id = r.assertion_group_id
        WHERE r.changeset_id = ?
        ORDER BY g.created_at DESC, g.id DESC
        "#,
        row.id,
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(CoreErrorContext::from)?;
    Ok(Some(ChangesetDetails {
        uuid,
        author: row.author,
        description: row.description,
        state: ChangesetState::from_db_str(&row.state)?,
        created_at: stored_timestamp(row.created_at),
        closed_at: row.closed_at.map(stored_timestamp),
        assertions,
        retractions,
    }))
}

use super::assertion_rows::{AssertionRow, group_rows};
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{Assertion, AssertionUuid, DataSourceUuid},
};

/// Accepted assertions that are still in effect, which new interpretations
/// include, newest first, with the files they're still in effect for.
pub async fn list_active_assertions(ctx: &CoreContext) -> Result<Vec<Assertion>, CoreError> {
    let rows = sqlx::query_as!(
        AssertionRow,
        r#"
        SELECT
            g.uuid AS "uuid!: AssertionUuid",
            g.created_at,
            g.rationale,
            g.confidence AS "confidence: f64",
            a.claim,
            ds.uuid AS "data_source_uuid!: DataSourceUuid",
            ds.name AS data_source_name,
            a.rel_path,
            a.retracted_at
        FROM assertions a
        JOIN assertion_groups g
            ON g.id = a.group_id
        JOIN changesets c
            ON c.id = g.changeset_id
        JOIN data_sources ds
            ON ds.id = a.data_source_id
        WHERE a.retracted_at IS NULL AND c.state = 'accepted'
        ORDER BY g.created_at DESC, g.id DESC, a.id
        "#
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;
    group_rows(rows)
}

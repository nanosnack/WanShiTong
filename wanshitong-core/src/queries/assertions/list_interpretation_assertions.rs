use super::assertion_rows::{AssertionRow, group_rows};
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{Assertion, AssertionUuid, DataSourceUuid, InterpretationRowId},
};

/// The assertions an interpretation was derived from, with the files it
/// included them for, whether they've been retracted since or not.
pub async fn list_interpretation_assertions(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
) -> Result<Vec<Assertion>, CoreError> {
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
        FROM interpretation_assertions ia
        JOIN assertions a
            ON a.id = ia.assertion_id
        JOIN assertion_groups g
            ON g.id = a.group_id
        JOIN data_sources ds
            ON ds.id = a.data_source_id
        WHERE ia.interpretation_id = ?
        ORDER BY g.created_at DESC, g.id DESC, a.id
        "#,
        interpretation_id,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;
    group_rows(rows)
}

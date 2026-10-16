use super::assertion_rows::{AssertionRow, group_rows};
use crate::{
    CoreError, CoreErrorContext,
    domain::{Assertion, AssertionUuid, ChangesetRowId, DataSourceUuid},
};

/// The assertions made in a changeset, newest first, with each file's
/// retraction, e.g. when it was withdrawn while the changeset was open.
pub(crate) async fn list_changeset_assertions(
    conn: &mut sqlx::SqliteConnection,
    changeset_id: ChangesetRowId,
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
        FROM assertions a
        JOIN assertion_groups g
            ON g.id = a.group_id
        JOIN data_sources ds
            ON ds.id = a.data_source_id
        WHERE g.changeset_id = ?
        ORDER BY g.created_at DESC, g.id DESC, a.id
        "#,
        changeset_id,
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(CoreErrorContext::from)?;
    group_rows(rows)
}

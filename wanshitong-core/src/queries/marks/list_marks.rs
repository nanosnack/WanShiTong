use crate::time::stored_timestamp;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{DataSourceUuid, Mark, MarkProgress, MarkUuid, MarkedFile},
};

/// Marks, newest first, with how far correcting each has got: pending while
/// an open changeset asserts about any of its files, and done once accepted
/// assertions about all of them were made since it was.
pub async fn list_marks(ctx: &CoreContext) -> Result<Vec<Mark>, CoreError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            m.uuid AS "uuid!: MarkUuid",
            m.label,
            m.note,
            m.created_at,
            ds.uuid AS "data_source_uuid!: DataSourceUuid",
            ds.name AS data_source_name,
            mf.rel_path,
            EXISTS (
                SELECT 1
                FROM assertions a
                JOIN assertion_groups g
                    ON g.id = a.group_id
                JOIN changesets c
                    ON c.id = g.changeset_id
                WHERE
                    a.data_source_id = mf.data_source_id
                    AND a.rel_path = mf.rel_path
                    AND c.state = 'accepted'
                    AND a.created_at >= m.created_at
            ) AS "done!: bool",
            EXISTS (
                SELECT 1
                FROM assertions a
                JOIN assertion_groups g
                    ON g.id = a.group_id
                JOIN changesets c
                    ON c.id = g.changeset_id
                WHERE
                    a.data_source_id = mf.data_source_id
                    AND a.rel_path = mf.rel_path
                    AND c.state = 'open'
                    AND a.retracted_at IS NULL
            ) AS "pending!: bool"
        FROM marks m
        JOIN mark_files mf
            ON mf.mark_id = m.id
        JOIN data_sources ds
            ON ds.id = mf.data_source_id
        ORDER BY m.created_at DESC, m.id DESC, ds.name, mf.rel_path
        "#,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;

    let mut marks: Vec<(Mark, bool, bool)> = Vec::new();
    for row in rows {
        let file = MarkedFile {
            data_source_uuid: row.data_source_uuid,
            data_source_name: row.data_source_name,
            rel_path: row.rel_path.into(),
        };
        match marks.last_mut() {
            Some((mark, all_done, any_pending)) if mark.uuid == row.uuid => {
                mark.files.push(file);
                *all_done &= row.done;
                *any_pending |= row.pending;
            }
            _ => marks.push((
                Mark {
                    uuid: row.uuid,
                    label: row.label,
                    note: row.note,
                    created_at: stored_timestamp(row.created_at),
                    files: vec![file],
                    progress: MarkProgress::Open,
                },
                row.done,
                row.pending,
            )),
        }
    }
    Ok(marks
        .into_iter()
        .map(|(mut mark, all_done, any_pending)| {
            mark.progress = match (all_done, any_pending) {
                (true, _) => MarkProgress::Done,
                (false, true) => MarkProgress::Pending,
                (false, false) => MarkProgress::Open,
            };
            mark
        })
        .collect())
}

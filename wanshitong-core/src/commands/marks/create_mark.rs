use crate::domain::PathId;
use crate::{CoreContext, CoreError, CoreErrorContext, domain::MarkUuid, time::unix_now_i64};
use uuid::Uuid;

pub struct NewMark {
    /// What was marked, for display, e.g. "movie Submarine D-1".
    pub label: String,
    pub note: String,
    /// The files that need reviewing, by data source and path.
    pub files: Vec<PathId>,
}

/// Marks files as needing correction.
pub async fn create_mark(ctx: &CoreContext, mark: NewMark) -> Result<MarkUuid, CoreError> {
    let NewMark { label, note, files } = mark;
    if files.is_empty() {
        error_stack::bail!(CoreErrorContext::MarkWithoutFiles);
    }
    let label = label.trim();
    let note = note.trim();
    let uuid = MarkUuid(Uuid::new_v4());
    let now = unix_now_i64();

    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;
    let mark_id = sqlx::query_scalar!(
        r#"
        INSERT INTO marks (uuid, label, note, created_at)
        VALUES (?, ?, ?, ?)
        RETURNING id AS "id!"
        "#,
        uuid,
        label,
        note,
        now,
    )
    .fetch_one(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;
    for PathId {
        data_source,
        rel_path,
    } in files
    {
        let rel_path = rel_path.to_string_lossy().into_owned();
        let data_source_id = sqlx::query_scalar!(
            r#"SELECT id AS "id!" FROM data_sources WHERE uuid = ?"#,
            data_source
        )
        .fetch_optional(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?
        .ok_or(CoreErrorContext::DataSourceNotFound(data_source))?;
        sqlx::query!(
            r#"
            INSERT INTO mark_files (mark_id, data_source_id, rel_path)
            VALUES (?, ?, ?)
            ON CONFLICT DO NOTHING
            "#,
            mark_id,
            data_source_id,
            rel_path,
        )
        .execute(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?;
    }
    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(uuid)
}

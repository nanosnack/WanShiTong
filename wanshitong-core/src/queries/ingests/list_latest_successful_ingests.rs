use crate::domain::{DataSourceUuid, IngestUuid};
use crate::{CoreContext, CoreError, CoreErrorContext};

/// The latest successful ingest of each undeleted data source, oldest data
/// source first. Data sources without one are left out.
pub async fn list_latest_successful_ingests(
    ctx: &CoreContext,
) -> Result<Vec<(DataSourceUuid, IngestUuid)>, CoreError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            ds.uuid AS "data_source_uuid!: DataSourceUuid",
            i.uuid AS "ingest_uuid!: IngestUuid"
        FROM data_sources ds
        JOIN (
            SELECT data_source_id, MAX(id) AS ingest_id
            FROM ingests
            WHERE success = 1
            GROUP BY data_source_id
        ) latest
            ON latest.data_source_id = ds.id
        JOIN ingests i
            ON i.id = latest.ingest_id
        WHERE ds.deleted_at IS NULL
        ORDER BY ds.created_at ASC
        "#
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;

    Ok(rows
        .into_iter()
        .map(|row| (row.data_source_uuid, row.ingest_uuid))
        .collect())
}

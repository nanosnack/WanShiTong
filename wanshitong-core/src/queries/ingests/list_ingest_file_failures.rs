use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{DataSourceUuid, IngestFailureStage, IngestFileFailure, IngestUuid},
};

/// The files an ingest saw but couldn't measure, in the order it met them.
pub async fn list_ingest_file_failures(
    ctx: &CoreContext,
    ingest: IngestUuid,
) -> Result<Vec<IngestFileFailure>, CoreError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            ds.uuid AS "data_source!: DataSourceUuid",
            f.rel_path,
            f.stage AS "stage!: IngestFailureStage",
            f.error
        FROM ingest_file_failures f
        JOIN ingests i ON i.id = f.ingest_id
        JOIN data_sources ds ON ds.id = i.data_source_id
        WHERE i.uuid = ?
        ORDER BY f.id
        "#,
        ingest,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;
    Ok(rows
        .into_iter()
        .map(|row| IngestFileFailure {
            data_source: row.data_source,
            rel_path: row.rel_path.into(),
            stage: row.stage,
            error: row.error,
        })
        .collect())
}

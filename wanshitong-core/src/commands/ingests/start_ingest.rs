use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{
        Config, DataSourceRowId, DataSourceUuid, FileMediaMeasurementRowId, IngestRowId, IngestUuid,
    },
    queries::get_config,
    time::unix_now_i64,
};
use sqlx::SqliteTransaction;
use std::{collections::HashMap, path::PathBuf};
use uuid::Uuid;

/// Records that an ingest of one data source has started.
pub async fn start_ingest(
    ctx: &CoreContext,
    data_source_uuid: DataSourceUuid,
) -> Result<NewIngest, CoreError> {
    let ingest_uuid = IngestUuid(Uuid::new_v4());
    let started_at = unix_now_i64();

    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;

    let (
        config_snap_id,
        Config {
            ffprobe_path,
            ffprobe_command,
            ..
        },
    ) = get_config(&mut txn).await?;
    let data_source_id = get_data_source_row_id(&mut txn, data_source_uuid).await?;

    // Create the ingest record.
    let ingest_row_id: IngestRowId = sqlx::query_scalar!(
        r#"
        INSERT INTO ingests (uuid, data_source_id, started_at, config_id)
        VALUES (?, ?, ?, ?)
        RETURNING id AS "id!: _"
        "#,
        ingest_uuid,
        data_source_id,
        started_at,
        config_snap_id
    )
    .fetch_one(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;

    // Fetch (size, mtime, media measurement) for every file in the data
    // source so we can avoid re-probing unchanged files.
    //
    // Reuse makes ingests share media measurements. See the schema's notes on
    // file_measurements.media_measurement_id.
    let prev_ingest_id: Option<IngestRowId> = sqlx::query_scalar!(
        r#"
        SELECT id AS "id!: _"
        FROM ingests
        WHERE data_source_id = ? AND success = 1
        ORDER BY id DESC
        LIMIT 1;"#,
        data_source_id
    )
    .fetch_optional(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;

    let mut prev_media_measurements = HashMap::new();
    if let Some(prev_ingest_id) = prev_ingest_id {
        let rows = sqlx::query!(
            r#"
            SELECT
                rel_path,
                size,
                mtime,
                media_measurement_id AS "media_measurement_id: FileMediaMeasurementRowId"
            FROM complete_file_measurements m
            WHERE m.data_source_id = ? AND m.ingest_id = ?;
            "#,
            data_source_id,
            prev_ingest_id,
        )
        .fetch_all(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?;

        for row in rows {
            prev_media_measurements.insert(
                row.rel_path.into(),
                PreviousMediaMeasurement {
                    size_bytes: row.size as u64,
                    mtime: row.mtime,
                    media_measurement_id: row.media_measurement_id,
                },
            );
        }
    }

    txn.commit().await.map_err(CoreErrorContext::from)?;

    Ok(NewIngest {
        uuid: ingest_uuid,
        row_id: ingest_row_id,
        data_source: NewIngestDataSource {
            uuid: data_source_uuid,
            row_id: data_source_id,
            prev_media_measurements,
        },
        ffprobe_path,
        ffprobe_command,
    })
}

async fn get_data_source_row_id(
    txn: &mut SqliteTransaction<'_>,
    uuid: DataSourceUuid,
) -> Result<DataSourceRowId, CoreError> {
    let source = sqlx::query!(
        r#"
        SELECT id AS "id!: DataSourceRowId", deleted_at
        FROM data_sources
        WHERE uuid = ?
        "#,
        uuid
    )
    .fetch_optional(&mut **txn)
    .await
    .map_err(CoreErrorContext::from)?
    .ok_or(CoreErrorContext::DataSourceNotFound(uuid))?;
    if source.deleted_at.is_some() {
        error_stack::bail!(CoreErrorContext::DataSourceDeleted(uuid));
    }
    Ok(source.id)
}

#[derive(Debug)]
pub struct NewIngest {
    pub uuid: IngestUuid,
    pub row_id: IngestRowId,
    pub data_source: NewIngestDataSource,
    pub ffprobe_path: String,
    pub ffprobe_command: String,
}

#[derive(Debug)]
pub struct NewIngestDataSource {
    pub uuid: DataSourceUuid,
    pub row_id: DataSourceRowId,
    pub prev_media_measurements: HashMap<PathBuf, PreviousMediaMeasurement>,
}

#[derive(Debug)]
pub struct PreviousMediaMeasurement {
    pub size_bytes: u64,
    pub mtime: i64,
    /// `None` means a probe occurred but the file was not a media file.
    pub media_measurement_id: Option<FileMediaMeasurementRowId>,
}

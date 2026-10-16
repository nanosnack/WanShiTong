//! Loading ingests with their data sources, for getting one or listing all.

use crate::time::stored_timestamp;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{
        DataSourceConfig, DataSourceUuid, FileSystemSource, Ingest, IngestDataSource, IngestRowId,
        IngestStats, IngestUuid, IngestWithDataSource,
    },
};

/// Every ingest, newest first, or just the one with `uuid`, each with its
/// data source. Only filesystem data sources exist yet.
pub(super) async fn load_ingests(
    ctx: &CoreContext,
    uuid: Option<IngestUuid>,
) -> Result<Vec<IngestWithDataSource>, CoreError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            i.id AS "id!: IngestRowId",
            i.uuid AS "uuid!: IngestUuid",
            i.started_at,
            i.files_seen,
            i.files_probed,
            i.files_recorded,
            i.media_files_recorded,
            i.files_failed,
            i.stopped_at,
            i.success,
            ds.uuid AS "data_source_uuid!: DataSourceUuid",
            ds.name AS data_source_name,
            ds.created_at AS data_source_created_at,
            fs.root_path,
            fs.follow_symlinks
        FROM ingests i
        JOIN data_sources ds
            ON ds.id = i.data_source_id
        JOIN fs_data_sources fs
            ON fs.id = ds.id
        WHERE ?1 IS NULL OR i.uuid = ?1
        ORDER BY i.started_at DESC
        "#,
        uuid,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;

    Ok(rows
        .into_iter()
        .map(|row| IngestWithDataSource {
            ingest: Ingest {
                id: row.id,
                uuid: row.uuid,
                started_at: stored_timestamp(row.started_at),
                stats: IngestStats {
                    files_seen: row.files_seen as u64,
                    files_probed: row.files_probed as u64,
                    files_recorded: row.files_recorded as u64,
                    media_files_recorded: row.media_files_recorded as u64,
                    files_failed: row.files_failed as u64,
                },
                stopped_at: row.stopped_at.map(stored_timestamp),
                success: row.success.unwrap_or(0) != 0,
            },
            data_source: IngestDataSource {
                uuid: row.data_source_uuid,
                name: row.data_source_name,
                config: DataSourceConfig::FileSystem(FileSystemSource {
                    root_path: row.root_path.into(),
                    follow_symlinks: row.follow_symlinks != 0,
                }),
                created_at: stored_timestamp(row.data_source_created_at),
            },
        })
        .collect())
}

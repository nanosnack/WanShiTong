//! Loading data sources, for getting one or listing the undeleted or the
//! deleted.

use crate::time::stored_timestamp;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{DataSource, DataSourceConfig, DataSourceUuid, FileSystemSource, IngestUuid},
};

/// Which data sources to load.
pub(super) enum DataSourceFilter {
    /// The one with this UUID, deleted or not.
    Uuid(DataSourceUuid),
    Undeleted,
    Deleted,
}

/// The data sources `filter` selects, oldest first, each with its latest
/// ingest. Only filesystem data sources exist yet.
pub(super) async fn load_data_sources(
    ctx: &CoreContext,
    filter: DataSourceFilter,
) -> Result<Vec<DataSource>, CoreError> {
    let (uuid, deleted) = match filter {
        DataSourceFilter::Uuid(uuid) => (Some(uuid), None),
        DataSourceFilter::Undeleted => (None, Some(false)),
        DataSourceFilter::Deleted => (None, Some(true)),
    };
    let rows = sqlx::query!(
        r#"
        SELECT
            ds.uuid AS "uuid!: DataSourceUuid",
            ds.name,
            ds.created_at,
            ds.deleted_at,
            fs.root_path,
            fs.follow_symlinks,

            i.uuid AS "latest_ingest_uuid?: IngestUuid"
        FROM data_sources ds
        JOIN fs_data_sources fs
            ON fs.id = ds.id

        LEFT JOIN (
            SELECT data_source_id, MAX(id) AS ingest_id
            FROM ingests
            GROUP BY data_source_id
        ) latest_ingest
            ON latest_ingest.data_source_id = ds.id
        LEFT JOIN ingests i
            ON i.id = latest_ingest.ingest_id

        WHERE
            ds.kind = 'filesystem'
            AND (?1 IS NULL OR ds.uuid = ?1)
            AND (?2 IS NULL OR (ds.deleted_at IS NOT NULL) = ?2)
        ORDER BY ds.created_at ASC
        "#,
        uuid,
        deleted,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;

    Ok(rows
        .into_iter()
        .map(|row| DataSource {
            uuid: row.uuid,
            name: row.name,
            config: DataSourceConfig::FileSystem(FileSystemSource {
                root_path: row.root_path.into(),
                follow_symlinks: row.follow_symlinks != 0,
            }),
            created_at: stored_timestamp(row.created_at),
            deleted_at: row.deleted_at.map(stored_timestamp),
            latest_ingest: row.latest_ingest_uuid,
        })
        .collect())
}

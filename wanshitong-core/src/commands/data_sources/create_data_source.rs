use super::conflicts::{check_no_view_overlap, name_error};
use crate::time::stored_timestamp;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{
        DataSource, DataSourceConfig, DataSourceKind, DataSourceRowId, DataSourceUuid,
        FileSystemSource,
    },
    time::unix_now_i64,
    validation::validate_name,
};
use uuid::Uuid;

pub async fn create_data_source(
    ctx: &CoreContext,
    name: impl AsRef<str>,
    config: DataSourceConfig,
) -> Result<DataSource, CoreError> {
    let name = name.as_ref().trim().to_string();
    validate_name(&name).map_err(CoreErrorContext::InvalidDataSourceName)?;

    let uuid = DataSourceUuid(Uuid::new_v4());
    let created_at = unix_now_i64();

    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;

    let root_path = match &config {
        DataSourceConfig::FileSystem(FileSystemSource { root_path, .. }) => root_path,
    };
    check_no_view_overlap(&mut txn, root_path).await?;

    let kind = DataSourceKind::FileSystem.as_db_str();
    let result: Result<DataSourceRowId, _> = sqlx::query_scalar!(
        r#"
        INSERT INTO data_sources (uuid, name, kind, created_at)
        VALUES (?, ?, ?, ?)
        RETURNING id AS "id: _"
        "#,
        uuid,
        name,
        kind,
        created_at
    )
    .fetch_one(&mut *txn)
    .await;

    let row_id = match result {
        Ok(i) => i,
        Err(e) => return Err(name_error(e, name)),
    };

    match &config {
        DataSourceConfig::FileSystem(FileSystemSource {
            root_path,
            follow_symlinks,
        }) => {
            let root_path_str = root_path.to_string_lossy();

            sqlx::query!(
                "INSERT INTO fs_data_sources (id, root_path, follow_symlinks) VALUES (?, ?, ?)",
                row_id,
                root_path_str,
                follow_symlinks
            )
            .execute(&mut *txn)
            .await
            .map_err(CoreErrorContext::from)?;
        }
    }

    txn.commit().await.map_err(CoreErrorContext::from)?;

    Ok(DataSource {
        uuid,
        name,
        config,
        created_at: stored_timestamp(created_at),
        deleted_at: None,
        latest_ingest: None,
    })
}

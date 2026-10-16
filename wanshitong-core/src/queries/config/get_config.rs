use crate::{
    CoreError, CoreErrorContext,
    domain::{AnyConfig, Config, ConfigSnapshotRowId},
};
use sqlx::SqliteTransaction;

pub(crate) async fn get_config(
    txn: &mut SqliteTransaction<'_>,
) -> Result<(ConfigSnapshotRowId, Config), CoreError> {
    let config_snap_id: ConfigSnapshotRowId = sqlx::query_scalar!(
        r#"
        SELECT latest_config_snapshot_id AS "latest_config_snapshot_id!: _"
        FROM system_state
        "#
    )
    .fetch_one(&mut **txn)
    .await
    .map_err(CoreErrorContext::from)?;
    let config_json = sqlx::query_scalar!(
        r#"
        SELECT config
        FROM config_snapshots
        WHERE id = ?
        "#,
        config_snap_id
    )
    .fetch_one(&mut **txn)
    .await
    .map_err(CoreErrorContext::from)?;
    let config: AnyConfig = serde_json::from_str(&config_json)
        .map_err(|e| CoreErrorContext::InvalidConfig(e, config_json))?;
    Ok((config_snap_id, config.upgrade()))
}

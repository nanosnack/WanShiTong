use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{Config, VersionedConfig},
    queries::get_config,
    time::unix_now_i64,
};

/// Makes `config` the one new ingests run with, as a new snapshot, unless the
/// latest snapshot already is it. Returns whether a snapshot was taken.
///
/// Snapshots are never changed in place: an ingest records the one it ran
/// with.
pub async fn set_config(ctx: &CoreContext, config: &Config) -> Result<bool, CoreError> {
    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;

    let (_, current) = get_config(&mut txn).await?;
    if current == *config {
        return Ok(false);
    }

    let created_at = unix_now_i64();
    let config_json = serde_json::to_string(&VersionedConfig::new(config.clone())).unwrap();
    sqlx::query!(
        r#"
        INSERT INTO config_snapshots (created_at, config)
        VALUES (?, ?)
        "#,
        created_at,
        config_json
    )
    .execute(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;
    sqlx::query!(
        r#"
        UPDATE system_state
        SET latest_config_snapshot_id = last_insert_rowid()
        WHERE id = 1
        "#
    )
    .execute(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;

    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(true)
}

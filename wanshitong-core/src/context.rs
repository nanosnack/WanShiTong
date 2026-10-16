use crate::{
    CoreError, CoreErrorContext,
    domain::{Config, VersionedConfig},
    time::unix_now_i64,
};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use std::{path::Path, time::Duration};

/// Reads and writes go through separate pools. `writer` has one connection, so
/// in-process writers queue in turn (sqlx pools hand out connections first
/// come, first served) instead of racing for SQLite's lock. SQLite's busy
/// handler has no queue, and a deferred transaction that reads before it
/// writes fails outright with `SQLITE_BUSY` if another writer commits in
/// between.
///
/// Any transaction that writes must use `writer`, even if it reads first.
#[derive(Clone)]
pub struct CoreContext {
    pub(crate) reader: SqlitePool,
    pub(crate) writer: SqlitePool,
}

impl CoreContext {
    pub async fn new_in_memory() -> Result<Self, sqlx::Error> {
        // Every connection to `sqlite::memory:` is its own database, so both
        // pools are the same single connection.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;

        sqlx::migrate!().run(&pool).await?;
        init_system_state(&pool).await?;

        Ok(Self {
            reader: pool.clone(),
            writer: pool,
        })
    }

    pub async fn new_in_file(data_dir: impl AsRef<Path>) -> Result<Self, CoreError> {
        let db_path = data_dir.as_ref().join("db.sqlite");
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .busy_timeout(Duration::from_millis(5000));
        let writer = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone().create_if_missing(true))
            .await
            .map_err(CoreErrorContext::from)?;

        sqlx::migrate!()
            .run(&writer)
            .await
            .map_err(|e| CoreErrorContext::Database(e.into()))?;
        init_system_state(&writer)
            .await
            .map_err(CoreErrorContext::from)?;

        // A write through `reader` would bring back the race `writer` exists
        // to prevent, so make it an error.
        let reader = SqlitePoolOptions::new()
            .connect_with(options.pragma("query_only", "ON"))
            .await
            .map_err(CoreErrorContext::from)?;

        Ok(Self { reader, writer })
    }
}

async fn init_system_state(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let mut txn = pool.begin().await?;

    let existing = sqlx::query_scalar!(
        r#"
        SELECT 1
        FROM system_state
        LIMIT 1
        "#
    )
    .fetch_optional(&mut *txn)
    .await?;

    if existing.is_some() {
        // Already initialized — idempotent no-op
        txn.commit().await?;
        return Ok(());
    }

    // Do first-time install.
    let created_at = unix_now_i64();
    let config = Config::default();
    let config_json = serde_json::to_string(&VersionedConfig::new(config)).unwrap();
    sqlx::query!(
        r#"
        INSERT INTO config_snapshots (created_at, config)
        VALUES (?, ?);
        "#,
        created_at,
        config_json
    )
    .execute(&mut *txn)
    .await?;
    sqlx::query!(
        r#"
        INSERT INTO system_state (latest_config_snapshot_id)
        VALUES (last_insert_rowid());
        "#
    )
    .execute(&mut *txn)
    .await?;

    txn.commit().await?;
    Ok(())
}

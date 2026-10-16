use crate::{CoreContext, CoreError, CoreErrorContext, domain::DataSourceUuid};
use sqlx::SqliteConnection;
use std::path::{Path, PathBuf};

/// The index file of the disc that a file is one of the other files of, in
/// its data source's latest successful ingest, if it is one. See *Disc* in
/// `spec/domain-model.md`.
pub async fn disc_of_file(
    ctx: &CoreContext,
    data_source: DataSourceUuid,
    rel_path: &Path,
) -> Result<Option<PathBuf>, CoreError> {
    let mut conn = ctx.reader.acquire().await.map_err(CoreErrorContext::from)?;
    let data_source_id = sqlx::query_scalar!(
        r#"SELECT id AS "id!" FROM data_sources WHERE uuid = ?"#,
        data_source
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(CoreErrorContext::from)?;
    let Some(data_source_id) = data_source_id else {
        return Ok(None);
    };
    disc_index_of(&mut conn, data_source_id, &rel_path.to_string_lossy()).await
}

pub(crate) async fn disc_index_of(
    conn: &mut SqliteConnection,
    data_source_id: i64,
    rel_path: &str,
) -> Result<Option<PathBuf>, CoreError> {
    let index = sqlx::query_scalar!(
        r#"
        SELECT ix.rel_path AS "index_path!"
        FROM file_measurements fm
        JOIN disc_files df
            ON df.file_id = fm.id
        JOIN discs d
            ON d.id = df.disc_id
        JOIN file_measurements ix
            ON ix.id = d.index_file_id
        WHERE
            fm.data_source_id = ?1
            AND fm.rel_path = ?2
            AND fm.ingest_id = (
                SELECT max(i.id)
                FROM ingests i
                WHERE i.data_source_id = ?1 AND i.success = 1
            )
        "#,
        data_source_id,
        rel_path,
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(CoreErrorContext::from)?;
    Ok(index.map(PathBuf::from))
}

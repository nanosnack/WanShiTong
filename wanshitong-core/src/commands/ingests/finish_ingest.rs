use super::record_discs::record_discs;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{IngestRowId, IngestStats},
    time::unix_now_i64,
};

pub async fn finish_ingest(
    ctx: &CoreContext,
    ingest_id: IngestRowId,
    success: bool,
    stats: IngestStats,
) -> Result<(), CoreError> {
    let stopped_at = unix_now_i64();

    let IngestStats {
        files_seen,
        files_probed,
        files_recorded,
        media_files_recorded,
        files_failed,
    } = stats;
    let files_seen = files_seen as i64;
    let files_probed = files_probed as i64;
    let files_recorded = files_recorded as i64;
    let media_files_recorded = media_files_recorded as i64;
    let files_failed = files_failed as i64;

    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;

    sqlx::query!(
        "UPDATE ingests
         SET
             files_seen = ?,
             files_probed = ?,
             files_recorded = ?,
             media_files_recorded = ?,
             files_failed = ?,
             stopped_at = ?,
             success = ?
         WHERE id = ?",
        files_seen,
        files_probed,
        files_recorded,
        media_files_recorded,
        files_failed,
        stopped_at,
        success,
        ingest_id,
    )
    .execute(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;

    // A successful ingest's discs are recorded with it, so that no
    // interpretation sees its files without them.
    if success {
        record_discs(&mut txn, ingest_id).await?;
    }

    txn.commit().await.map_err(CoreErrorContext::from)?;

    Ok(())
}

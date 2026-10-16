use crate::{AppContext, AppError, AppErrorContext, ingest::IngestProgressSnapshot};
use wanshitong_core::domain::IngestUuid;

pub async fn wait_for_ingest(
    ctx: &AppContext,
    ingest_id: IngestUuid,
) -> Result<IngestProgressSnapshot, AppError> {
    let listener = ctx
        .ingest_progress
        .listen(ingest_id)
        .ok_or(AppErrorContext::IngestNotFound(ingest_id))?;
    loop {
        let snapshot = listener.poll().await;
        if snapshot.is_finished() {
            return Ok(snapshot);
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

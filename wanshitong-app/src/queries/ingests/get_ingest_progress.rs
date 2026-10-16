use crate::{AppContext, ingest::IngestProgressSnapshot};
use wanshitong_core::domain::IngestUuid;

pub async fn get_ingest_progress(
    ctx: &AppContext,
    ingest_id: IngestUuid,
) -> Option<IngestProgressSnapshot> {
    Some(ctx.ingest_progress.listen(ingest_id)?.poll().await)
}

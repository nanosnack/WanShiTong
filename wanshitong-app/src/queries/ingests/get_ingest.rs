use crate::ingest::IngestStatus;
use crate::queries::get_ingest_progress;
use crate::{AppContext, AppError};
use wanshitong_core as core;
use wanshitong_core::domain::{IngestUuid, IngestWithDataSource};

pub async fn get_ingest(
    ctx: &AppContext,
    id: IngestUuid,
) -> Result<Option<IngestStatus>, AppError> {
    let persisted = core::queries::get_ingest(&ctx.core, id).await?;
    if let Some(IngestWithDataSource {
        ingest,
        data_source,
    }) = persisted
    {
        let progress = get_ingest_progress(ctx, id).await;
        Ok(Some(IngestStatus {
            ingest,
            data_source,
            progress,
        }))
    } else {
        Ok(None)
    }
}

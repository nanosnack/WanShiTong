use crate::{AppContext, AppError, ingest::IngestStatus};
use wanshitong_core::{self as core, domain::IngestWithDataSource};

pub async fn list_ingests(ctx: &AppContext) -> Result<Vec<IngestStatus>, AppError> {
    let persisted_ingests = core::queries::list_ingests(&ctx.core).await?;

    // Enrich the persisted records with in-memory progress tracking.
    let mut rich_ingests = Vec::with_capacity(persisted_ingests.len());
    for IngestWithDataSource {
        ingest,
        data_source,
    } in persisted_ingests
    {
        let mut with_progress = IngestStatus {
            ingest,
            data_source,
            progress: None,
        };
        if let Some(listener) = ctx.ingest_progress.listen(ingest.uuid) {
            let progress = listener.poll().await;
            with_progress.progress = Some(progress);
        }
        rich_ingests.push(with_progress);
    }

    Ok(rich_ingests)
}

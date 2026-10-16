use crate::{AppContext, AppError};
use wanshitong_core::{
    self as core,
    domain::{IngestFileFailure, IngestUuid},
};

/// The files an ingest saw but couldn't measure, in the order it met them.
pub async fn list_ingest_file_failures(
    ctx: &AppContext,
    ingest: IngestUuid,
) -> Result<Vec<IngestFileFailure>, AppError> {
    Ok(core::queries::list_ingest_file_failures(&ctx.core, ingest).await?)
}

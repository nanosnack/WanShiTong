use super::ingest_rows::load_ingests;
use crate::{
    CoreContext, CoreError,
    domain::{IngestUuid, IngestWithDataSource},
};

/// The ingest with `uuid`, with its data source, or None if there's none.
pub async fn get_ingest(
    ctx: &CoreContext,
    uuid: IngestUuid,
) -> Result<Option<IngestWithDataSource>, CoreError> {
    Ok(load_ingests(ctx, Some(uuid)).await?.pop())
}

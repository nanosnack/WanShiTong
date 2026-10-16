use super::ingest_rows::load_ingests;
use crate::{CoreContext, CoreError, domain::IngestWithDataSource};

/// Every ingest, newest first, with its data source.
pub async fn list_ingests(ctx: &CoreContext) -> Result<Vec<IngestWithDataSource>, CoreError> {
    load_ingests(ctx, None).await
}

use super::data_source_rows::{DataSourceFilter, load_data_sources};
use crate::{CoreContext, CoreError, domain::DataSource};

/// Data sources that haven't been deleted, oldest first.
pub async fn list_data_sources(ctx: &CoreContext) -> Result<Vec<DataSource>, CoreError> {
    load_data_sources(ctx, DataSourceFilter::Undeleted).await
}

/// Deleted data sources, oldest first.
pub async fn list_deleted_data_sources(ctx: &CoreContext) -> Result<Vec<DataSource>, CoreError> {
    load_data_sources(ctx, DataSourceFilter::Deleted).await
}

use super::data_source_rows::{DataSourceFilter, load_data_sources};
use crate::{
    CoreContext, CoreError,
    domain::{DataSource, DataSourceUuid},
};

/// The data source with `uuid`, deleted or not, or None if there's none.
pub async fn get_data_source(
    ctx: &CoreContext,
    uuid: DataSourceUuid,
) -> Result<Option<DataSource>, CoreError> {
    Ok(load_data_sources(ctx, DataSourceFilter::Uuid(uuid))
        .await?
        .pop())
}

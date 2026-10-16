use crate::{AppContext, AppError};
use wanshitong_core::domain::{DataSource, DataSourceConfig};

pub async fn create_data_source(
    ctx: &AppContext,
    name: impl AsRef<str>,
    config: DataSourceConfig,
) -> Result<DataSource, AppError> {
    let source = wanshitong_core::commands::create_data_source(&ctx.core, name, config).await?;
    ctx.data_sources_changed.notify_one();
    Ok(source)
}

use crate::{AppContext, AppError};
use wanshitong_core::domain::{DataSource, DataSourceUuid};

pub async fn get_data_source(
    ctx: &AppContext,
    uuid: DataSourceUuid,
) -> Result<Option<DataSource>, AppError> {
    Ok(wanshitong_core::queries::get_data_source(&ctx.core, uuid).await?)
}

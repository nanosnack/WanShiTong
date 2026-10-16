use crate::{AppContext, AppError};
use wanshitong_core::domain::DataSource;

pub async fn list_data_sources(ctx: &AppContext) -> Result<Vec<DataSource>, AppError> {
    Ok(wanshitong_core::queries::list_data_sources(&ctx.core).await?)
}

pub async fn list_deleted_data_sources(ctx: &AppContext) -> Result<Vec<DataSource>, AppError> {
    Ok(wanshitong_core::queries::list_deleted_data_sources(&ctx.core).await?)
}

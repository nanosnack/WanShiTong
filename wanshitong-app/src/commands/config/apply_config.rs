use crate::{AppContext, AppError};
use tracing::info;
use wanshitong_core::commands::set_config;

/// Makes the config file's ingest settings, e.g. `ffprobe_path`, the ones new
/// ingests run with, if they changed since the last run. Done at startup,
/// before anything is ingested.
pub async fn apply_config(ctx: &AppContext) -> Result<(), AppError> {
    if set_config(&ctx.core, &ctx.config().core()).await? {
        info!("Ingest settings changed in the config file; new ingests use them");
    }
    Ok(())
}

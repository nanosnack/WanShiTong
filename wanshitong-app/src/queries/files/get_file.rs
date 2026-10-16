use crate::{AppContext, AppError};
use std::path::Path;
use wanshitong_core as core;
use wanshitong_core::domain::{DataSourceUuid, FileDetails};
use wanshitong_core::queries::get_canonical_interpretation_id;

/// The file as the canonical interpretation sees it, or None if there's no
/// canonical interpretation or it didn't see the file.
pub async fn get_file(
    ctx: &AppContext,
    data_source_uuid: DataSourceUuid,
    rel_path: &Path,
) -> Result<Option<FileDetails>, AppError> {
    let Some(interpretation_id) = get_canonical_interpretation_id(&ctx.core).await? else {
        return Ok(None);
    };
    Ok(core::queries::get_file(&ctx.core, interpretation_id, data_source_uuid, rel_path).await?)
}

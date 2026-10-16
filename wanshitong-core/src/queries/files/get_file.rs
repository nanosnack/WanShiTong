use super::list_file_details;
use crate::{
    CoreContext, CoreError,
    domain::{DataSourceUuid, FileDetails, InterpretationRowId, PathId},
};
use std::path::Path;

/// The file at `rel_path` in a data source, as measured by the interpretation's
/// ingests, or None if they didn't see it. For several files at once, see
/// [`list_file_details`].
pub async fn get_file(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
    data_source_uuid: DataSourceUuid,
    rel_path: &Path,
) -> Result<Option<FileDetails>, CoreError> {
    let path = PathId::new(data_source_uuid, rel_path);
    Ok(
        list_file_details(ctx, interpretation_id, std::slice::from_ref(&path))
            .await?
            .pop(),
    )
}

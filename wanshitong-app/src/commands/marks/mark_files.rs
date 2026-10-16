use crate::{AppContext, AppError};
use wanshitong_core::commands::{self as core, NewMark};
use wanshitong_core::domain::{MarkUuid, PathId};
use wanshitong_core::queries::disc_of_file;

/// Marks files as needing correction. A disc's other files are marked as the
/// disc, by its index file, which stands for them.
pub async fn mark_files(ctx: &AppContext, mut mark: NewMark) -> Result<MarkUuid, AppError> {
    let mut files = Vec::with_capacity(mark.files.len());
    for PathId {
        data_source,
        rel_path,
    } in mark.files
    {
        let rel_path = match disc_of_file(&ctx.core, data_source, &rel_path).await? {
            Some(index) => index,
            None => rel_path,
        };
        let file = PathId::new(data_source, rel_path);
        if !files.contains(&file) {
            files.push(file);
        }
    }
    mark.files = files;
    Ok(core::create_mark(&ctx.core, mark).await?)
}

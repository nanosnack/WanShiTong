use crate::{AppContext, AppError};
use wanshitong_core::queries::get_canonical_interpretation_id;
use wanshitong_core::{self as core};

pub use core::queries::{FileTreeCursor, FileTreeEntry, FileTreeList, FileTreeQuery};

/// A folder's children, as the canonical interpretation saw them.
pub async fn list_file_tree(
    ctx: &AppContext,
    query: &FileTreeQuery,
) -> Result<FileTreeList, AppError> {
    let Some(canonical_interpretation_id) = get_canonical_interpretation_id(&ctx.core).await?
    else {
        return Ok(FileTreeList {
            entries: Vec::new(),
            next: None,
        });
    };
    Ok(core::queries::list_file_tree(&ctx.core, canonical_interpretation_id, query).await?)
}

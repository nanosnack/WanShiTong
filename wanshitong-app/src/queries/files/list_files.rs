use crate::{AppContext, AppError};
use wanshitong_core::domain::InterpretationRowId;
use wanshitong_core::queries::get_canonical_interpretation_id;
use wanshitong_core::{self as core};

pub use core::queries::{FileCursor, FileSearchQuery, FileSummaryList};

/// The files the canonical interpretation saw.
pub async fn list_files(
    ctx: &AppContext,
    query: &FileSearchQuery,
) -> Result<FileSummaryList, AppError> {
    let Some(canonical_interpretation_id) = get_canonical_interpretation_id(&ctx.core).await?
    else {
        return Ok(FileSummaryList {
            files: Vec::new(),
            next_cursor: None,
        });
    };
    list_files_in(ctx, canonical_interpretation_id, query).await
}

/// The files an interpretation saw.
pub async fn list_files_in(
    ctx: &AppContext,
    interpretation_id: InterpretationRowId,
    query: &FileSearchQuery,
) -> Result<FileSummaryList, AppError> {
    Ok(core::queries::list_files(&ctx.core, interpretation_id, query).await?)
}

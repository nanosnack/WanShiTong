use crate::{AppContext, AppError};
use wanshitong_core::domain::ChangesetState;
use wanshitong_core::queries as core;

pub use core::ChangesetSummary;

/// Changesets, newest first, only those in `state` if given.
pub async fn list_changesets(
    ctx: &AppContext,
    state: Option<ChangesetState>,
) -> Result<Vec<ChangesetSummary>, AppError> {
    Ok(core::list_changesets(&ctx.core, state).await?)
}

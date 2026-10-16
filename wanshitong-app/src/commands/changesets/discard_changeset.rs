use crate::{AppContext, AppError};
use wanshitong_core::commands as core;
use wanshitong_core::domain::ChangesetUuid;

/// Drops an open changeset's pending changes.
pub async fn discard_changeset(ctx: &AppContext, changeset: ChangesetUuid) -> Result<(), AppError> {
    Ok(core::discard_changeset(&ctx.core, changeset).await?)
}

use crate::{AppContext, AppError};
use wanshitong_core::commands::{self as core, NewChangeset};
use wanshitong_core::domain::ChangesetUuid;

/// Opens a changeset to make assertions and retractions in.
pub async fn create_changeset(
    ctx: &AppContext,
    changeset: NewChangeset,
) -> Result<ChangesetUuid, AppError> {
    Ok(core::create_changeset(&ctx.core, changeset).await?)
}

use crate::commands::USER;
use crate::{AppContext, AppError};
use wanshitong_core::domain::{ChangesetState, ChangesetUuid};
use wanshitong_core::queries::list_changesets;

/// The draft, if there is one: the newest open changeset by the user.
pub async fn find_draft(ctx: &AppContext) -> Result<Option<ChangesetUuid>, AppError> {
    let open = list_changesets(&ctx.core, Some(ChangesetState::Open)).await?;
    Ok(open
        .into_iter()
        .find(|changeset| changeset.author == USER)
        .map(|changeset| changeset.uuid))
}

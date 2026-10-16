use crate::{AppContext, AppError, CoreErrorContext};
use wanshitong_core::commands as core;
use wanshitong_core::domain::{AssertionUuid, ChangesetUuid};
use wanshitong_core::queries::get_changeset;

/// Takes a change back out of an open changeset: withdraws one of its
/// assertions, or drops its retraction of an accepted one.
pub async fn take_back_from_changeset(
    ctx: &AppContext,
    changeset: ChangesetUuid,
    assertion: AssertionUuid,
) -> Result<(), AppError> {
    let own = get_changeset(&ctx.core, changeset)
        .await?
        .ok_or(CoreErrorContext::ChangesetNotFound(changeset))?
        .assertions
        .iter()
        .any(|own| own.uuid == assertion);
    if own {
        core::retract_assertion(&ctx.core, changeset, assertion).await?;
    } else {
        core::undo_retraction(&ctx.core, changeset, assertion).await?;
    }
    Ok(())
}

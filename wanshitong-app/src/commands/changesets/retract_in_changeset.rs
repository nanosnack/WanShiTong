use crate::{AppContext, AppError};
use wanshitong_core::commands as core;
use wanshitong_core::domain::{AssertionUuid, ChangesetUuid};

/// Retracts an assertion in an open changeset: one of its own at once, or an
/// accepted one once the changeset is accepted.
pub async fn retract_in_changeset(
    ctx: &AppContext,
    changeset: ChangesetUuid,
    assertion: AssertionUuid,
) -> Result<(), AppError> {
    Ok(core::retract_assertion(&ctx.core, changeset, assertion).await?)
}

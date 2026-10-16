use crate::{AppContext, AppError};
use wanshitong_core::commands::{self as core, NewAssertion};
use wanshitong_core::domain::{AssertionUuid, ChangesetUuid};

/// Makes assertions in an open changeset, all or none of them. Nothing is
/// interpreted until something asks what the changeset would change.
pub async fn add_assertions(
    ctx: &AppContext,
    changeset: ChangesetUuid,
    assertions: Vec<NewAssertion>,
) -> Result<Vec<AssertionUuid>, AppError> {
    Ok(core::create_assertions(&ctx.core, changeset, assertions).await?)
}

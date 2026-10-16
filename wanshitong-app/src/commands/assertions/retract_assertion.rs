use super::user_changeset::{describe, file_name, user_changeset};
use crate::commands::{accept_changeset, retract_in_changeset};
use crate::{AppContext, AppError};
use wanshitong_core::domain::{AssertionUuid, InterpretationUuid};
use wanshitong_core::queries::list_active_assertions;

/// Retracts an assertion, as the user, in a changeset of its own that's
/// accepted at once, then starts an interpretation without it, to become
/// canonical once it succeeds.
pub async fn retract_assertion(
    ctx: &AppContext,
    uuid: AssertionUuid,
) -> Result<InterpretationUuid, AppError> {
    let assertion = list_active_assertions(&ctx.core)
        .await?
        .into_iter()
        .find(|assertion| assertion.uuid == uuid);
    let name = match assertion {
        Some(assertion) => match assertion.files.as_slice() {
            [] => format!("Retracted {}", describe(&assertion.claim)),
            [file] => format!(
                "Retracted {} for \"{}\"",
                describe(&assertion.claim),
                file_name(&file.rel_path)
            ),
            [file, rest @ ..] => format!(
                "Retracted {} for \"{}\" and {} more",
                describe(&assertion.claim),
                file_name(&file.rel_path),
                rest.len()
            ),
        },
        None => "Retracted an assertion".to_string(),
    };
    let changeset = user_changeset(ctx, &name).await?;
    retract_in_changeset(ctx, changeset, uuid).await?;
    accept_changeset(ctx, changeset).await
}

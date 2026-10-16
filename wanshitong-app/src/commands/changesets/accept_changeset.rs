use super::changeset_interpretation::{interpretation_name, wait_for_interpretation};
use super::interpret_changeset::start_new_changeset_interpretation;
use crate::commands::change_canonical_interpretation;
use crate::interpretations::InterpretationState;
use crate::{AppContext, AppError};
use wanshitong_core::commands as core;
use wanshitong_core::domain::{ChangesetUuid, InterpretationUuid};
use wanshitong_core::queries::find_current_changeset_interpretation;

/// Applies an open changeset's changes, and makes an interpretation that
/// includes them canonical: the changeset's own, if it's still up to date,
/// and otherwise a new one, which becomes canonical once it succeeds.
pub async fn accept_changeset(
    ctx: &AppContext,
    changeset: ChangesetUuid,
) -> Result<InterpretationUuid, AppError> {
    core::accept_changeset(&ctx.core, changeset).await?;
    if let Some(uuid) = find_current_changeset_interpretation(&ctx.core, changeset).await? {
        let details = wait_for_interpretation(ctx, uuid).await?;
        if details.state == InterpretationState::Succeeded {
            change_canonical_interpretation(ctx, details.interpretation.id).await?;
            return Ok(uuid);
        }
    }
    // Recorded as the changeset's, so that it's known as what accepting made.
    let name = interpretation_name(ctx, changeset, None).await?;
    start_new_changeset_interpretation(ctx, changeset, &name, true).await
}

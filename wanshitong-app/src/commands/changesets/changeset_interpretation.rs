//! The interpretations that changesets make: naming one, and waiting for
//! one to finish.

use crate::commands::fit_name;
use crate::interpretations::{InterpretationState, InterpretationStatus};
use crate::queries::get_interpretation;
use crate::{AppContext, AppError, AppErrorContext, CoreErrorContext};
use std::time::Duration;
use wanshitong_core::domain::{ChangesetUuid, InterpretationUuid};
use wanshitong_core::queries::get_changeset;

/// The changeset's description, e.g. "Identify Perry Mason's TV movies",
/// after `prefix` if given, e.g. "Changeset: ...", to tell apart from the
/// interpretations that accepting changesets makes.
pub(super) async fn interpretation_name(
    ctx: &AppContext,
    changeset: ChangesetUuid,
    prefix: Option<&str>,
) -> Result<String, AppError> {
    let changeset = get_changeset(&ctx.core, changeset)
        .await?
        .ok_or(CoreErrorContext::ChangesetNotFound(changeset))?;
    let about = match changeset.description.as_str() {
        "" => format!("Changeset by {}", changeset.author),
        description => description.replace(char::is_control, " "),
    };
    Ok(fit_name(match prefix {
        Some(prefix) => format!("{prefix}: {about}"),
        None => about,
    }))
}

/// Polls until the interpretation stops running.
pub(super) async fn wait_for_interpretation(
    ctx: &AppContext,
    uuid: InterpretationUuid,
) -> Result<InterpretationStatus, AppError> {
    loop {
        let details = get_interpretation(ctx, uuid)
            .await?
            .ok_or(AppErrorContext::InterpretationNotFound(uuid))?;
        if details.state != InterpretationState::Running {
            return Ok(details);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

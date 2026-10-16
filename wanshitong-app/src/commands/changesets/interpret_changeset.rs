use super::changeset_interpretation::{interpretation_name, wait_for_interpretation};
use crate::commands::spawn_interpretation;
use crate::interpretations::{InterpretationState, InterpretationStatus};
use crate::queries::get_interpretation;
use crate::{AppContext, AppError, AppErrorContext};
use wanshitong_core::commands as core;
use wanshitong_core::domain::{ChangesetUuid, InterpretationUuid};
use wanshitong_core::queries::{
    find_current_changeset_interpretation, list_latest_successful_ingests,
};

/// An interpretation of what accepting an open changeset would make: its
/// latest, if it's up to date, or else a new one. Waits for it to finish, as
/// it's for comparing with the canonical catalog.
pub async fn interpret_changeset(
    ctx: &AppContext,
    changeset: ChangesetUuid,
) -> Result<InterpretationStatus, AppError> {
    let uuid = start_changeset_interpretation(ctx, changeset).await?;
    wait_for_interpretation(ctx, uuid).await
}

/// Starts interpreting what accepting an open changeset would make, unless
/// its latest interpretation is up to date, without waiting for it, e.g. for
/// a page to show while it runs.
pub async fn start_changeset_interpretation(
    ctx: &AppContext,
    changeset: ChangesetUuid,
) -> Result<InterpretationUuid, AppError> {
    if let Some(uuid) = current_changeset_interpretation(ctx, changeset).await? {
        return Ok(uuid);
    }
    let name = interpretation_name(ctx, changeset, Some("Changeset")).await?;
    start_new_changeset_interpretation(ctx, changeset, &name, false).await
}

/// Starts a new interpretation of a changeset, named `name`, and recorded as
/// the changeset's. If `make_canonical`, it becomes canonical once it
/// succeeds, which only an accepted changeset's can.
pub(super) async fn start_new_changeset_interpretation(
    ctx: &AppContext,
    changeset: ChangesetUuid,
    name: &str,
    make_canonical: bool,
) -> Result<InterpretationUuid, AppError> {
    let sources = list_latest_successful_ingests(&ctx.core).await?;
    let new_interpretation =
        core::create_changeset_interpretation(&ctx.core, name, &sources, changeset).await?;
    let uuid = new_interpretation.uuid;
    spawn_interpretation(ctx, new_interpretation, make_canonical).await;
    Ok(uuid)
}

/// The open changeset's latest interpretation, if it's up to date and running
/// or finished: one that accepting the changeset now would reuse.
pub async fn current_changeset_interpretation(
    ctx: &AppContext,
    changeset: ChangesetUuid,
) -> Result<Option<InterpretationUuid>, AppError> {
    let Some(uuid) = find_current_changeset_interpretation(&ctx.core, changeset).await? else {
        return Ok(None);
    };
    let details = get_interpretation(ctx, uuid)
        .await?
        .ok_or(AppErrorContext::InterpretationNotFound(uuid))?;
    // One interrupted, e.g. by a restart, won't finish.
    Ok((details.state != InterpretationState::Interrupted).then_some(uuid))
}

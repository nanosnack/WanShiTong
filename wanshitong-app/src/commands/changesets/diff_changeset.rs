use crate::commands::interpret_changeset;
use crate::queries::{InterpretationDiff, diff_interpretations};
use crate::{AppContext, AppError};
use wanshitong_core::domain::ChangesetUuid;

/// What accepting an open changeset would change: its interpretation, made
/// on demand, compared with the canonical catalog.
pub async fn diff_changeset(
    ctx: &AppContext,
    changeset: ChangesetUuid,
) -> Result<InterpretationDiff, AppError> {
    let interpretation = interpret_changeset(ctx, changeset).await?;
    diff_interpretations(ctx, None, interpretation.interpretation.uuid).await
}

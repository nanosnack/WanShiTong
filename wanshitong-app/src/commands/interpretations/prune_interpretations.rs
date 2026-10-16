use crate::{AppContext, AppError};
use jiff::{SignedDuration, Timestamp};
use tracing::info;
use wanshitong_core::commands as core;

/// Deletes the interpretations older than the configured retention, but for
/// those still in use: the canonical one, any still running, and an open
/// changeset's. Runs at startup and after each interpretation finishes, so
/// the database stops growing with every correction. Returns how many were
/// deleted.
pub async fn prune_interpretations(ctx: &AppContext) -> Result<usize, AppError> {
    let days = ctx.config().interpretation_retention_days;
    // Capped at a thousand years, which keeps everything, so that no setting
    // overflows the duration.
    let hours = i64::try_from(days.min(365_000)).unwrap() * 24;
    // A cutoff that can't be computed prunes nothing.
    let before = Timestamp::now()
        .saturating_sub(SignedDuration::from_hours(hours))
        .unwrap_or(Timestamp::MIN);
    let deleted = core::prune_interpretations(&ctx.core, before).await?;
    if !deleted.is_empty() {
        info!(
            "Deleted {} interpretation(s) older than {} day(s)",
            deleted.len(),
            days
        );
    }
    Ok(deleted.len())
}

use super::catalog_diff::{diff_catalogs, diff_unidentified};
use crate::interpretations::InterpretationStatus;
use crate::queries::{UnidentifiedChange, WorkChange, get_interpretation_or_canonical};
use crate::{AppContext, AppError};
use wanshitong_core::domain::InterpretationUuid;
use wanshitong_core::queries::list_catalog_placements;

/// What differs between two interpretations' catalogs.
pub struct InterpretationDiff {
    pub base: InterpretationStatus,
    pub other: InterpretationStatus,
    /// Sorted by the works' titles.
    pub changes: Vec<WorkChange>,
    pub unidentified: UnidentifiedChange,
}

/// Compares `other`'s catalog with `base`'s, the canonical one by default.
pub async fn diff_interpretations(
    ctx: &AppContext,
    base: Option<InterpretationUuid>,
    other: InterpretationUuid,
) -> Result<InterpretationDiff, AppError> {
    let base = get_interpretation_or_canonical(ctx, base).await?;
    let other = get_interpretation_or_canonical(ctx, Some(other)).await?;
    let base_placements = list_catalog_placements(&ctx.core, base.interpretation.id).await?;
    let other_placements = list_catalog_placements(&ctx.core, other.interpretation.id).await?;
    let changes = diff_catalogs(&base_placements, &other_placements);
    let unidentified = diff_unidentified(&base_placements, &other_placements, &changes);
    Ok(InterpretationDiff {
        base,
        other,
        changes,
        unidentified,
    })
}

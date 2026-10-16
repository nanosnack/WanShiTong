use crate::commands::{add_assertions, open_draft, retract_in_changeset};
use crate::{AppContext, AppError};
use std::collections::HashSet;
use wanshitong_core::commands::NewAssertion;
use wanshitong_core::domain::{AssertionUuid, ClaimPredicate, PathId};
use wanshitong_core::queries::{get_changeset, list_active_assertions};

/// Asserts each of `claims` about all of `files` in the draft. Assertions they
/// would conflict with are taken back in the same draft, so that no two
/// assertions contradict each other: an accepted one is retracted, and one of
/// the draft's own withdrawn. Only those wholly about `files` are, so that
/// correcting some files never undoes what was asserted about others.
pub async fn correct_in_draft(
    ctx: &AppContext,
    files: &[PathId],
    claims: Vec<ClaimPredicate>,
) -> Result<Vec<AssertionUuid>, AppError> {
    let draft = open_draft(ctx).await?;
    let correcting: HashSet<&PathId> = files.iter().collect();
    let conflicts = |claim: &ClaimPredicate| claims.iter().any(|c| c.conflicts_with(claim));

    let accepted = list_active_assertions(&ctx.core).await?;
    let pending = get_changeset(&ctx.core, draft)
        .await?
        .map(|draft| draft.assertions)
        .unwrap_or_default();
    for assertion in accepted.iter().chain(&pending) {
        let in_effect: Vec<_> = assertion
            .files
            .iter()
            .filter(|file| file.retracted_at.is_none())
            .map(|file| PathId::new(file.data_source_uuid, &file.rel_path))
            .collect();
        if !in_effect.is_empty()
            && in_effect.iter().all(|file| correcting.contains(file))
            && conflicts(&assertion.claim)
        {
            retract_in_changeset(ctx, draft, assertion.uuid).await?;
        }
    }

    let assertions = claims
        .into_iter()
        .map(|claim| NewAssertion {
            files: files.to_vec(),
            claim,
            rationale: None,
            confidence: None,
        })
        .collect();
    add_assertions(ctx, draft, assertions).await
}

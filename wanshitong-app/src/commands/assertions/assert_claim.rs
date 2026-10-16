use super::user_changeset::{describe, file_name, user_changeset};
use crate::commands::{accept_changeset, add_assertions};
use crate::{AppContext, AppError, CoreErrorContext};
use wanshitong_core::commands::NewAssertion;
use wanshitong_core::domain::{ClaimPredicate, InterpretationUuid, PathId};

/// Asserts a claim about each of `files`, as the user, in a changeset of its
/// own that's accepted at once, then starts one interpretation that includes
/// it, to become canonical once it succeeds. Both are named after the first
/// file.
///
/// To correct a movie's identity, assert it about all of the movie's files, so
/// they keep belonging to one movie.
pub async fn assert_claim(
    ctx: &AppContext,
    files: &[PathId],
    claim: ClaimPredicate,
) -> Result<InterpretationUuid, AppError> {
    let name = match files {
        [] => return Err(CoreErrorContext::AssertionWithoutFiles.into()),
        [PathId { rel_path, .. }] => format!(
            "Asserted {} for \"{}\"",
            describe(&claim),
            file_name(rel_path)
        ),
        [PathId { rel_path, .. }, rest @ ..] => format!(
            "Asserted {} for \"{}\" and {} more",
            describe(&claim),
            file_name(rel_path),
            rest.len()
        ),
    };
    let changeset = user_changeset(ctx, &name).await?;
    add_assertions(
        ctx,
        changeset,
        vec![NewAssertion {
            files: files.to_vec(),
            claim,
            rationale: None,
            confidence: None,
        }],
    )
    .await?;
    accept_changeset(ctx, changeset).await
}

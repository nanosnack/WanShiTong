use crate::commands::{USER, create_changeset, find_draft};
use crate::{AppContext, AppError};
use wanshitong_core::commands::NewChangeset;
use wanshitong_core::domain::ChangesetUuid;

/// The draft, opened if there isn't one.
pub async fn open_draft(ctx: &AppContext) -> Result<ChangesetUuid, AppError> {
    match find_draft(ctx).await? {
        Some(draft) => Ok(draft),
        None => {
            create_changeset(
                ctx,
                NewChangeset {
                    author: USER.into(),
                    description: "Corrections in the web UI".into(),
                },
            )
            .await
        }
    }
}

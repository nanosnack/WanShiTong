use crate::{AppContext, AppError};
use wanshitong_core::domain::{ChangesetDetails, ChangesetUuid};
use wanshitong_core::queries as core;

pub async fn get_changeset(
    ctx: &AppContext,
    uuid: ChangesetUuid,
) -> Result<Option<ChangesetDetails>, AppError> {
    Ok(core::get_changeset(&ctx.core, uuid).await?)
}

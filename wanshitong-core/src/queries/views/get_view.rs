use super::view_rows::fetch_views;
use crate::domain::{View, ViewUuid};
use crate::{CoreContext, CoreError};

pub async fn get_view(ctx: &CoreContext, uuid: ViewUuid) -> Result<Option<View>, CoreError> {
    Ok(fetch_views(ctx, Some(uuid)).await?.pop())
}

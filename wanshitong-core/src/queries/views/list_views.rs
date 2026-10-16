use super::view_rows::fetch_views;
use crate::{CoreContext, CoreError, domain::View};

pub async fn list_views(ctx: &CoreContext) -> Result<Vec<View>, CoreError> {
    fetch_views(ctx, None).await
}

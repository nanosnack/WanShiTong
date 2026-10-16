use crate::views::ViewStatus;
use crate::{AppContext, AppError};
use wanshitong_core::queries::get_canonical_interpretation_id;

pub async fn list_views(ctx: &AppContext) -> Result<Vec<ViewStatus>, AppError> {
    let canonical = get_canonical_interpretation_id(&ctx.core).await?;
    let views = wanshitong_core::queries::list_views(&ctx.core).await?;
    Ok(views
        .into_iter()
        .map(|view| ViewStatus::new(view, canonical))
        .collect())
}

use crate::views::ViewStatus;
use crate::{AppContext, AppError};
use wanshitong_core::domain::ViewUuid;
use wanshitong_core::queries::get_canonical_interpretation_id;

pub async fn get_view(ctx: &AppContext, uuid: ViewUuid) -> Result<Option<ViewStatus>, AppError> {
    let canonical = get_canonical_interpretation_id(&ctx.core).await?;
    let view = wanshitong_core::queries::get_view(&ctx.core, uuid).await?;
    Ok(view.map(|view| ViewStatus::new(view, canonical)))
}

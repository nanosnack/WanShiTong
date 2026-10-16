use crate::{AppContext, AppError};
use wanshitong_core::{
    self as core,
    domain::{InterpretationError, InterpretationUuid},
};

/// What an interpretation couldn't do and went on without, in the order it
/// was recorded.
pub async fn list_interpretation_errors(
    ctx: &AppContext,
    interpretation: InterpretationUuid,
) -> Result<Vec<InterpretationError>, AppError> {
    Ok(core::queries::list_interpretation_errors(&ctx.core, interpretation).await?)
}

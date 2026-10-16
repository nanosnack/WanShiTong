use crate::{
    CoreContext, CoreError, CoreErrorContext, domain::InterpretationRowId, time::unix_now_i64,
};

/// Marks an interpretation finished: successfully if `error` is `None`.
pub async fn finish_interpretation(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
    error: Option<&str>,
) -> Result<(), CoreError> {
    let stopped_at = unix_now_i64();
    sqlx::query!(
        "UPDATE interpretations SET stopped_at = ?, error = ? WHERE id = ?",
        stopped_at,
        error,
        interpretation_id,
    )
    .execute(&ctx.writer)
    .await
    .map_err(CoreErrorContext::from)?;
    Ok(())
}

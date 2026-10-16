use crate::{CoreContext, CoreError, CoreErrorContext};

/// Marks step `step` (counting from 0) done, if it's the one the guide is on.
/// Otherwise, as when the same step is submitted twice, does nothing.
pub async fn complete_onboarding_step(ctx: &CoreContext, step: u32) -> Result<(), CoreError> {
    sqlx::query!(
        r#"
        UPDATE system_state
        SET onboarding_steps_done = onboarding_steps_done + 1
        WHERE id = 1 AND onboarding_steps_done = ?
        "#,
        step,
    )
    .execute(&ctx.writer)
    .await
    .map_err(CoreErrorContext::from)?;
    Ok(())
}

/// Goes back to step `step` (counting from 0), if the guide is on the step
/// after it. Otherwise, as when the same step is submitted twice, does
/// nothing.
pub async fn return_to_onboarding_step(ctx: &CoreContext, step: u32) -> Result<(), CoreError> {
    sqlx::query!(
        r#"
        UPDATE system_state
        SET onboarding_steps_done = ?1
        WHERE id = 1 AND onboarding_steps_done = ?1 + 1
        "#,
        step,
    )
    .execute(&ctx.writer)
    .await
    .map_err(CoreErrorContext::from)?;
    Ok(())
}

use crate::{CoreContext, CoreError, CoreErrorContext};

/// Shows or hides the guide. Showing it again once all `step_count` steps are
/// done starts it over.
pub async fn set_onboarding_enabled(
    ctx: &CoreContext,
    enabled: bool,
    step_count: u32,
) -> Result<(), CoreError> {
    sqlx::query!(
        r#"
        UPDATE system_state
        SET
            onboarding_enabled = ?1,
            onboarding_steps_done = CASE
                WHEN ?1 AND onboarding_steps_done >= ?2 THEN 0
                ELSE onboarding_steps_done
            END
        WHERE id = 1
        "#,
        enabled,
        step_count,
    )
    .execute(&ctx.writer)
    .await
    .map_err(CoreErrorContext::from)?;
    Ok(())
}

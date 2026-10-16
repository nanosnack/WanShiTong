use crate::{CoreContext, CoreError, CoreErrorContext, domain::OnboardingState};

pub async fn get_onboarding(ctx: &CoreContext) -> Result<OnboardingState, CoreError> {
    let row = sqlx::query!(
        r#"
        SELECT
            onboarding_enabled AS "enabled: bool",
            onboarding_steps_done AS "steps_done: u32"
        FROM system_state
        WHERE id = 1
        "#
    )
    .fetch_one(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;
    Ok(OnboardingState {
        enabled: row.enabled,
        steps_done: row.steps_done,
    })
}

use wanshitong_core::CoreContext;
use wanshitong_core::commands::{
    complete_onboarding_step, return_to_onboarding_step, set_onboarding_enabled,
};
use wanshitong_core::domain::OnboardingState;
use wanshitong_core::queries::get_onboarding;

const STEPS: u32 = 3;

async fn state(ctx: &CoreContext) -> OnboardingState {
    get_onboarding(ctx).await.unwrap()
}

#[tokio::test]
async fn a_new_database_starts_the_guide() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    assert_eq!(
        state(&ctx).await,
        OnboardingState {
            enabled: true,
            steps_done: 0
        }
    );
}

#[tokio::test]
async fn completing_a_step_other_than_the_current_one_does_nothing() {
    let ctx = CoreContext::new_in_memory().await.unwrap();

    complete_onboarding_step(&ctx, 0).await.unwrap();
    assert_eq!(state(&ctx).await.steps_done, 1);

    // Done submitted twice, or from a page open since before the first.
    complete_onboarding_step(&ctx, 0).await.unwrap();
    assert_eq!(state(&ctx).await.steps_done, 1);

    // A step not reached yet.
    complete_onboarding_step(&ctx, 2).await.unwrap();
    assert_eq!(state(&ctx).await.steps_done, 1);
}

#[tokio::test]
async fn showing_the_guide_again_starts_it_over_only_once_finished() {
    let ctx = CoreContext::new_in_memory().await.unwrap();

    complete_onboarding_step(&ctx, 0).await.unwrap();
    set_onboarding_enabled(&ctx, false, STEPS).await.unwrap();
    set_onboarding_enabled(&ctx, true, STEPS).await.unwrap();
    assert_eq!(
        state(&ctx).await,
        OnboardingState {
            enabled: true,
            steps_done: 1
        }
    );

    for step in 1..STEPS {
        complete_onboarding_step(&ctx, step).await.unwrap();
    }
    assert_eq!(state(&ctx).await.steps_done, STEPS);
    set_onboarding_enabled(&ctx, true, STEPS).await.unwrap();
    assert_eq!(state(&ctx).await.steps_done, 0);
}

#[tokio::test]
async fn returning_to_a_step_only_goes_back_one_from_the_current_step() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    for step in 0..2 {
        complete_onboarding_step(&ctx, step).await.unwrap();
    }

    return_to_onboarding_step(&ctx, 1).await.unwrap();
    assert_eq!(state(&ctx).await.steps_done, 1);

    // Previous submitted twice.
    return_to_onboarding_step(&ctx, 1).await.unwrap();
    assert_eq!(state(&ctx).await.steps_done, 1);

    // Not the step before the current one.
    return_to_onboarding_step(&ctx, 2).await.unwrap();
    assert_eq!(state(&ctx).await.steps_done, 1);
}

//! The first-run guide: a welcome, then a few steps, done in order, that teach
//! a new user the app. The user says when each is done; nothing checks.

use crate::{AppContext, AppError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnboardingStep {
    /// Choosing whether to follow the guide at all. Until this is done, the
    /// guide hasn't started.
    Welcome,
    CreateDataSource,
    AwaitCatalog,
    ReviewCatalog,
    LearnCorrections,
    CreateView,
}

impl OnboardingStep {
    pub const ALL: [OnboardingStep; 6] = [
        OnboardingStep::Welcome,
        OnboardingStep::CreateDataSource,
        OnboardingStep::AwaitCatalog,
        OnboardingStep::ReviewCatalog,
        OnboardingStep::LearnCorrections,
        OnboardingStep::CreateView,
    ];

    /// Counting from 0.
    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|&step| step == self).unwrap() as u32
    }

    pub fn from_index(index: u32) -> Option<Self> {
        Self::ALL.get(index as usize).copied()
    }

    pub fn is_last(self) -> bool {
        Self::ALL.last() == Some(&self)
    }

    /// The step's number as shown to the user, counting from 1 and not
    /// counting the welcome, which is `None`.
    pub fn number(self) -> Option<u32> {
        (self != Self::Welcome).then_some(self.index())
    }

    /// How many steps there are to show, not counting the welcome.
    pub const NUMBERED: u32 = Self::ALL.len() as u32 - 1;
}

/// What decided whether the guide is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnboardingSetBy {
    /// `onboarding` in the config file, which the UI can't change.
    ConfigFile,
    /// The Config page (or the default).
    Database,
}

#[derive(Clone, Copy, Debug)]
pub struct Onboarding {
    pub enabled: bool,
    pub set_by: OnboardingSetBy,
    /// The step the guide is on, or `None` once all are done.
    pub step: Option<OnboardingStep>,
}

impl Onboarding {
    /// The step to show, if the guide is shown.
    pub fn current_step(&self) -> Option<OnboardingStep> {
        self.step.filter(|_| self.enabled)
    }
}

pub async fn get_onboarding(ctx: &AppContext) -> Result<Onboarding, AppError> {
    let state = wanshitong_core::queries::get_onboarding(&ctx.core).await?;
    let (enabled, set_by) = match ctx.config().onboarding {
        Some(enabled) => (enabled, OnboardingSetBy::ConfigFile),
        None => (state.enabled, OnboardingSetBy::Database),
    };
    Ok(Onboarding {
        enabled,
        set_by,
        step: OnboardingStep::from_index(state.steps_done),
    })
}

/// Shows or hides the guide, starting it over if it was finished. Has no effect
/// while the config file decides.
pub async fn set_onboarding_enabled(ctx: &AppContext, enabled: bool) -> Result<(), AppError> {
    wanshitong_core::commands::set_onboarding_enabled(
        &ctx.core,
        enabled,
        OnboardingStep::ALL.len() as u32,
    )
    .await?;
    Ok(())
}

/// Moves the guide past `step`, if it's the one it's on.
pub async fn complete_onboarding_step(
    ctx: &AppContext,
    step: OnboardingStep,
) -> Result<(), AppError> {
    wanshitong_core::commands::complete_onboarding_step(&ctx.core, step.index()).await?;
    Ok(())
}

/// Moves the guide back to `step`, if it's on the one after.
pub async fn return_to_onboarding_step(
    ctx: &AppContext,
    step: OnboardingStep,
) -> Result<(), AppError> {
    wanshitong_core::commands::return_to_onboarding_step(&ctx.core, step.index()).await?;
    Ok(())
}

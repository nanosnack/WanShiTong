/// The stored state of the first-run guide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnboardingState {
    pub enabled: bool,
    /// How many of the guide's steps are done, in order.
    pub steps_done: u32,
}

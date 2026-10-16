//! The commands: what can be done to the domain. Each subject module lists
//! its commands, one per file, named for it.

mod assertions;
mod catalog;
mod changesets;
mod config;
mod data_sources;
mod ingests;
mod interpretations;
mod marks;
mod onboarding;
mod views;

pub use assertions::*;
pub use catalog::*;
pub use changesets::*;
pub use config::*;
pub use data_sources::*;
pub use ingests::*;
pub use interpretations::*;
pub use marks::*;
pub use onboarding::*;
pub use views::*;

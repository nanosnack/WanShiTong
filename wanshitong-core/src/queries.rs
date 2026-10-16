//! The queries: what can be asked of the domain. Each subject module lists
//! its queries, one per file, named for it.

mod assertions;
mod catalog;
mod changesets;
mod config;
mod data_sources;
mod files;
mod ingests;
mod interpretations;
mod marks;
mod onboarding;
mod views;

pub use assertions::*;
pub use catalog::*;
pub use changesets::*;
pub(crate) use config::*;
pub use data_sources::*;
pub use files::*;
pub use ingests::*;
pub use interpretations::*;
pub use marks::*;
pub use onboarding::*;
pub use views::*;

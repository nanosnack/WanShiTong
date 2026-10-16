//! The commands: what the app can be asked to do. Each subject module lists
//! its commands, one per file, named for it.

mod assertions;
mod changesets;
mod config;
mod data_sources;
mod ingests;
mod interpretations;
mod marks;
mod tasks;
mod views;

pub use assertions::*;
pub use changesets::*;
pub use config::*;
pub use data_sources::*;
pub use ingests::*;
pub use interpretations::*;
pub use marks::*;
pub use tasks::*;
pub use views::*;
pub use wanshitong_core::commands::{NewAssertion, NewChangeset, NewMark};

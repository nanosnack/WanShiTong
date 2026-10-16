#![doc = include_str!("../README.md")]

pub mod commands;
pub mod domain;
pub mod queries;

mod build;
mod changesets;
mod context;
mod error;
mod json;
mod sort_key;
mod time;
mod validation;

pub use build::current_build;
pub use context::CoreContext;
pub use error::{CoreError, CoreErrorContext};
pub use validation::{MAX_NAME_LEN, NameError};

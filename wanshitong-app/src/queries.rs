//! The queries: what the app can be asked. Each subject module lists its
//! queries, one per file, named for it.

mod assertions;
mod catalog;
mod changesets;
mod data_sources;
mod files;
mod ingests;
mod interpretations;
mod knowledge_base;
mod marks;
mod views;

pub use assertions::*;
pub use catalog::*;
pub use changesets::*;
pub use data_sources::*;
pub use files::*;
pub use ingests::*;
pub use interpretations::*;
pub use knowledge_base::*;
pub use marks::*;
pub use views::*;

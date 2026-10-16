//! Knowledge base lookups for people and agents exploring what a work might
//! be. They go through WST so that they see what interpretation sees, with
//! its cache and credentials, but they make no claims.

mod get_kb_movie;
mod get_kb_tv_series;
mod search_knowledge_base;

pub use get_kb_movie::{KbMovie, get_kb_movie};
pub use get_kb_tv_series::{KbSeason, KbTvSeries, get_kb_tv_series};
pub use search_knowledge_base::{KbSearchHit, KbWorkKind, search_knowledge_base};

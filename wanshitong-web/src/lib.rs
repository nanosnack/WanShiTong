#![doc = include_str!("../README.md")]
// Views nest as deeply as their pages do, e.g. a series page, a season, a
// file, its audit log and a table, which exceeds the default limit when
// checking that the nested futures are `Send`.
#![recursion_limit = "256"]

mod api;
mod api_docs;
mod app;
mod context;
mod cursor;
mod display;
mod error;
mod icons;
mod statics;
mod styles;
// Copied from topcoat's UI registry, which ships more variants than this app uses.
#[allow(dead_code)]
mod ui;

use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing::Level;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;

pub use context::{AppContext, CoreContext, WebContext};
pub use wanshitong_app::commands::{
    apply_config, prune_interpretations, seed_data_sources, watch_data_sources,
};
pub use wanshitong_app::{AppError, Config, report_error};

pub const WST_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The JSON API and the web UI as an axum router.
///
/// `/api/v1` is the JSON API, and `/api/docs` its reference; every other
/// request is handled by the topcoat app.
pub fn router(state: WebContext) -> axum::Router {
    let (api, openapi) = OpenApiRouter::with_openapi(api::ApiDoc::openapi())
        .nest("/api/v1", api::router(state.clone()))
        .split_for_parts();
    api.merge(api_docs::router(openapi))
        .fallback_service(topcoat::router::tower::TowerService::new(app::router(
            state,
        )))
        // A span per request, at a level that's on by default, so that an
        // error report's span trace says which request it failed.
        .layer(TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::new().level(Level::INFO)))
}

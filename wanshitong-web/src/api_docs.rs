//! The API's reference: [Scalar](https://scalar.com/) at `/api/docs`, reading
//! the OpenAPI document at `/api/v1/openapi.json`.
//!
//! Scalar's browser bundle is vendored in `assets/`, rather than loaded from a
//! CDN, so the reference works offline. To bump it, see `assets/README.md`.

use axum::{
    Router,
    body::Bytes,
    http::header::{CACHE_CONTROL, CONTENT_TYPE},
    response::{Html, IntoResponse},
    routing::get,
};
use tower_http::compression::CompressionLayer;

pub(crate) const DOCS_PATH: &str = "/api/docs";
const SPEC_PATH: &str = "/api/v1/openapi.json";

/// Versioned, so a bump changes the URL and the bundle can be cached forever.
const SCALAR_JS_PATH: &str = "/api/docs/scalar-standalone-1.66.1.js";
const SCALAR_JS: &[u8] = include_bytes!("../assets/scalar-standalone-1.66.1.js");

pub(crate) fn router(openapi: utoipa::openapi::OpenApi) -> Router {
    let spec = Bytes::from(
        openapi
            .to_json()
            .expect("an OpenAPI document built by utoipa serializes to JSON"),
    );
    Router::new()
        .route(DOCS_PATH, get(|| async { Html(page()) }))
        .route(
            SCALAR_JS_PATH,
            // The bundle is ~4 MB, a quarter of that gzipped. Only gzip: a
            // brotli pass at its default quality would take seconds.
            get(scalar_js).layer(CompressionLayer::new().no_br().no_deflate().no_zstd()),
        )
        .route(
            SPEC_PATH,
            get(move || async move { ([(CONTENT_TYPE, "application/json")], spec) }),
        )
}

async fn scalar_js() -> impl IntoResponse {
    (
        [
            (CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        SCALAR_JS,
    )
}

fn page() -> String {
    let config = serde_json::json!({
        "url": SPEC_PATH,
        // Scalar's hosted services, which it offers on localhost: they'd send
        // the API to scalar.com.
        "agent": { "disabled": true },
        "mcp": { "disabled": true },
        "showToolbar": "never",
        "showDeveloperTools": "never",
        "telemetry": false,
    });
    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>API reference · Wan Shi Tong</title>
    <link rel="icon" type="image/png" href="/static/favicon-96x96.png" sizes="96x96">
    <link rel="shortcut icon" href="/static/favicon.ico">
  </head>
  <body>
    <div id="app"></div>
    <script src="{SCALAR_JS_PATH}"></script>
    <script>
      // The document has no servers, so without a base URL the examples' URLs
      // are only paths.
      Scalar.createApiReference('#app', {{ ...{config}, baseServerURL: location.origin }});
    </script>
  </body>
</html>
"#
    )
}

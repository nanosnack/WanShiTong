//! Files embedded from `static/` at build time, and the ones `build.rs`
//! generates.

use include_dir::{Dir, include_dir};
use topcoat::{
    Result,
    context::Cx,
    router::{Body, error::RouterErrorExt, request::uri, response::Response, route},
};

static STATIC_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/static");

/// Where the layout links the stylesheet.
pub const STYLESHEET_PATH: &str = "/static/app.css";
/// Where the layout loads `ts/theme.ts`, compiled.
pub const THEME_SCRIPT_PATH: &str = "/static/theme.js";
/// Where the layout loads `ts/copy.ts`, compiled.
pub const COPY_SCRIPT_PATH: &str = "/static/copy.js";
/// Where the layout loads `ts/dialog.ts`, compiled.
pub const DIALOG_SCRIPT_PATH: &str = "/static/dialog.js";
/// Where the layout loads `ts/posters.ts`, compiled.
pub const POSTERS_SCRIPT_PATH: &str = "/static/posters.js";
/// Where the layout loads `ts/selection.ts`, compiled.
pub const SELECTION_SCRIPT_PATH: &str = "/static/selection.js";
/// Where the layout loads `ts/tree.ts`, compiled.
pub const TREE_SCRIPT_PATH: &str = "/static/tree.js";
/// Where the layout loads `ts/nav.ts`, compiled.
pub const NAV_SCRIPT_PATH: &str = "/static/nav.js";
/// Where the layout loads `ts/groups.ts`, compiled.
pub const GROUPS_SCRIPT_PATH: &str = "/static/groups.js";

/// Files `build.rs` generates: the stylesheet it compiles from `styles.css` and
/// the Tailwind classes in `src/`, and the scripts it compiles from `ts/`.
/// Served here rather than through topcoat's asset bundle, which would need a
/// separate post-build bundling step.
static GENERATED: &[(&str, &str, &str)] = &[
    (
        STYLESHEET_PATH,
        "text/css",
        include_str!(concat!(env!("OUT_DIR"), "/tailwind.css")),
    ),
    (
        THEME_SCRIPT_PATH,
        "text/javascript",
        include_str!(concat!(env!("OUT_DIR"), "/ts/theme.js")),
    ),
    (
        COPY_SCRIPT_PATH,
        "text/javascript",
        include_str!(concat!(env!("OUT_DIR"), "/ts/copy.js")),
    ),
    (
        DIALOG_SCRIPT_PATH,
        "text/javascript",
        include_str!(concat!(env!("OUT_DIR"), "/ts/dialog.js")),
    ),
    (
        POSTERS_SCRIPT_PATH,
        "text/javascript",
        include_str!(concat!(env!("OUT_DIR"), "/ts/posters.js")),
    ),
    (
        SELECTION_SCRIPT_PATH,
        "text/javascript",
        include_str!(concat!(env!("OUT_DIR"), "/ts/selection.js")),
    ),
    (
        TREE_SCRIPT_PATH,
        "text/javascript",
        include_str!(concat!(env!("OUT_DIR"), "/ts/tree.js")),
    ),
    (
        NAV_SCRIPT_PATH,
        "text/javascript",
        include_str!(concat!(env!("OUT_DIR"), "/ts/nav.js")),
    ),
    (
        GROUPS_SCRIPT_PATH,
        "text/javascript",
        include_str!(concat!(env!("OUT_DIR"), "/ts/groups.js")),
    ),
];

#[route(GET "/static/{*file}")]
async fn static_file(cx: &Cx) -> Result<Response> {
    let path = uri(cx).path();
    if let Some((_, mime, contents)) = GENERATED.iter().find(|(p, ..)| *p == path) {
        return Ok(Response::builder()
            .header("Content-Type", *mime)
            .body(Body::from(*contents))?);
    }

    let path = path.trim_start_matches("/static/");
    let file = STATIC_DIR.get_file(path).ok_or_not_found()?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();

    Ok(Response::builder()
        .header("Content-Type", mime.as_ref())
        .body(Body::from(file.contents()))?)
}

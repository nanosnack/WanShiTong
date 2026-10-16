`wanshitong-web` provides the HTTP interface and user-facing web UI.

This crate is responsible for:
- Defining HTTP routes and request/response handling
- Rendering HTML views and handling user input
- Translating HTTP interactions into application-level calls
- Managing web-specific concerns such as routing, forms, and redirects

The web layer contains no domain logic or workflow orchestration.
It delegates all meaningful operations to `wanshitong-app` and renders
the results for the user.

The external JSON API is under `/api/v1`, in `src/api`, with plain axum
handlers. Its request and response types are in `wanshitong-api`, which
clients share. Each handler's `#[utoipa::path]` documents it, and
`src/api_docs.rs` serves the OpenAPI document at `/api/v1/openapi.json`
and a [Scalar](https://scalar.com/) reference for it at `/api/docs`.

Pages are server-rendered with [topcoat](https://github.com/tokio-rs/topcoat),
organized as a module router: each module under `src/app` adds a URL
segment, and a `path_param!` module captures one (`app/movies/movie_id.rs`
is `/movies/{movie_id}`). A module that only splits up a page's code declares
`segment!(kind = Group)` and adds no segment (`app/corrections/marking.rs`
posts to `/corrections/mark`). Links and forms are plain HTML and work
without JavaScript.

Where a page needs the browser, it uses topcoat's client runtime first, which
the layout loads on every page: signals with `$(...)` expressions, `@event`
handlers and `:attribute` bindings hold local UI state, a procedure makes a
server call from a handler, a shard is re-rendered by the server as a signal
it reads changes, and a `live!` region is pushed over a WebSocket. An
expression is a subset of Rust (see topcoat's `expr!` docs); `raw!` embeds a
line of JavaScript for what it lacks, such as scrolling an element into view.

[Alpine.js](https://alpinejs.dev) with [Alpine AJAX](https://alpine-ajax.js.org),
vendored in `static/`, is only for what the runtime can't do: swapping a
form's response into part of the page (`x-target`, which keeps the form
working without JavaScript and can push the URL), appending pages of results
(`x-merge="append"` with `x-intersect`), and mounting the components in `ts/`
that work on the DOM directly. Don't reach for it where a signal would hold
the state, nor for what an inline attribute does (`onsubmit="return
confirm(…)"`). Both can run on one page, but an element is one library's:
Alpine directives don't go inside a shard or a `live!` region, which topcoat
re-renders.

The runtime, as of topcoat 0.9, has limits worth knowing before writing an
expression: there is no `&&`, `||`, `Some` or `None` (nest `if`s, use
`then_some`); a `return` inside an `if` only leaves that `if`, which compiles
to a function of its own; an `expr!` closure can't be called from another
expression; a binding's value must be owned (`to_owned()` an indexed
element); and `topcoat fmt` breaks a `raw!` too long for its line into a form
that doesn't compile, so keep it short, in an `expr!` outside the view if
need be.

Styling uses [topcoat's UI components](https://docs.rs/topcoat/latest/topcoat/ui/)
and Tailwind. The components were copied into `src/ui` by `topcoat ui add`
(tracked in `components.toml`) and are ours to edit; the theme is
`styles.css`. `build.rs` compiles the theme and the classes used in `src/`
with the Tailwind CLI named by `TAILWIND_CLI` (the Nix dev shell sets it),
and `statics.rs` serves the result at `/static/app.css`. Classes shared by
pages are constants in `src/styles.rs`.

A page's own components live in its module. A component used by several
pages lives in the nearest module above all of them (`ingest_state` in
`app/ingests.rs` serves both ingest pages), or, if they are in unrelated
parts of the tree, with the page it describes (`data_source_summary` is in
`app/data_sources/data_source_id.rs`).

In `view!`, `@event` and `:attribute` are topcoat syntax. Write Alpine
directives in their long form (`x-on:click`, `x-bind:class`).

`topcoat fmt` (from `cargo install topcoat-cli`, at the version in
`Cargo.toml`) formats the bodies of `view!` and the other topcoat macros,
which rustfmt leaves alone; run it, then `cargo fmt`. It can't break a
string literal, so write a class list too long for one line as
`class=(class!("…", "…"))`: it lays the segments out one per line, and
they compile to a single string. Check its output around `live!` blocks,
whose Rust it has mangled.

Script code is TypeScript, in `ts/`. `build.rs` type-checks and compiles it
with the `tsc` named by `TSC` (the Nix dev shell sets it), and `statics.rs`
serves the output from `/static/`. A component that needs more of the DOM
than a runtime expression reaches (observers, the clipboard, storage, the
file tree's cascading checkboxes) is an `Alpine.data` component there,
mounted with `x-data`, and so is anything more than a one-line Alpine
expression. `ts/theme.ts` also sets the color theme (the one picked in the
header, else the system's) before the page is painted.


//! The UI, organized as a topcoat module router: each module below this one
//! adds a URL segment (`app::data_sources` is `/data-sources`).
//!
//! Pages are server-rendered and styled with topcoat's UI components (copied
//! into `src/ui`) and Tailwind. Links and forms are plain HTML; where a page
//! needs the browser, topcoat's runtime comes first and Alpine.js with Alpine
//! AJAX covers what it can't do. The crate README says which is which.

mod about;
mod assertions;
mod catalog;
mod config;
mod corrections;
mod data_sources;
mod draft;
mod file_audit;
mod file_key;
mod files;
mod form_dialog;
mod ingests;
mod interpretations;
mod movies;
mod onboarding;
mod page_help;
mod path_completions;
mod path_tree;
mod tmdb_id;
mod tv_series;
mod views;

use crate::{
    api_docs,
    context::app,
    error::PageError,
    icons::{ARROW_UP_RIGHT, CHECK, MENU, MONITOR, MOON, SUN, lucide},
    statics::{
        COPY_SCRIPT_PATH, DIALOG_SCRIPT_PATH, GROUPS_SCRIPT_PATH, NAV_SCRIPT_PATH,
        POSTERS_SCRIPT_PATH, SELECTION_SCRIPT_PATH, STYLESHEET_PATH, THEME_SCRIPT_PATH,
        TREE_SCRIPT_PATH,
    },
    styles::H1,
    ui::{
        button::{ButtonSize, ButtonVariant, button_variants},
        dropdown_menu::{
            dropdown_menu, dropdown_menu_content, dropdown_menu_item, dropdown_menu_trigger,
        },
    },
};
use topcoat::{
    Result,
    context::Cx,
    cookie::RouterBuilderCookieExt,
    router::{
        BodyLimit, Router, RouterBuilderDiscoverExt, Slot, error::see_other, href, layout,
        module_router, not_found, page, request::uri,
    },
    runtime::RouterBuilderRuntimeExt,
    view::{Attributes, View, attributes, component, error_boundary, view},
};
use wanshitong_app::onboarding::{OnboardingStep, get_onboarding};

/// A link in the narrow-screen menu, styled like the menu's own items (which
/// are buttons).
const NAV_ITEM: &str = concat!(
    "flex w-full items-center rounded-md px-2 py-1.5 text-sm ",
    "hover:bg-foreground/5 active:bg-foreground/10",
);
const NAV_ITEM_CURRENT: &str =
    "flex w-full items-center rounded-md px-2 py-1.5 text-sm bg-foreground/5 font-medium";

/// How large an import's upload may be. An assertion is a few hundred bytes
/// of JSON, so this holds a library's worth many times over.
const IMPORT_BODY_LIMIT: usize = 256 * 1024 * 1024;

pub fn router(state: crate::WebContext) -> Router {
    module_router!()
        .discover()
        .layer(BodyLimit::max(IMPORT_BODY_LIMIT).at("/assertions/import"))
        .app_context(state)
        .runtime()
        .cookies()
        .build()
}

#[layout]
async fn layout(cx: &Cx, slot: Slot<'_>) -> Result<impl View> {
    // (label, URL, `aria-current`), to highlight the link to the current page
    // or the section it's in. Each `href!` target is its own type, hence
    // resolving them here.
    let path = uri(cx).path();
    let nav = [
        (
            "Catalog",
            href!(catalog::page).resolve(cx),
            // Movies' and series' pages aren't below the catalog's, but are its.
            &["/movies", "/tv-series"][..],
        ),
        ("Files", href!(files::page).resolve(cx), &[][..]),
        (
            "Corrections",
            href!(corrections::page).resolve(cx),
            // The section's other pages: the drafts being made, and the
            // assertions in effect.
            &["/draft", "/assertions"][..],
        ),
        (
            "Data Sources",
            href!(data_sources::page).resolve(cx),
            &[][..],
        ),
        ("Ingests", href!(ingests::page).resolve(cx), &[][..]),
        (
            "Interpretations",
            href!(interpretations::page).resolve(cx),
            &[][..],
        ),
        ("Views", href!(views::page).resolve(cx), &[][..]),
        ("Config", href!(config::page).resolve(cx), &[][..]),
        ("About", href!(about::page).resolve(cx), &[][..]),
    ]
    .map(|(label, url, also)| {
        let current = aria_current(path, &url, also);
        (label, url, current)
    });
    // Shown in the compact header: the page, or else our name, which
    // translators should leave alone.
    let current_label = nav
        .iter()
        .find(|(_, _, current)| current.is_some())
        .map(|(label, _, _)| *label);

    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">

                <link
                    rel="icon"
                    type="image/png"
                    href="/static/favicon-96x96.png"
                    sizes="96x96"
                >
                <link rel="icon" type="image/svg+xml" href="/static/favicon.svg">
                <link rel="shortcut icon" href="/static/favicon.ico">
                <link
                    rel="apple-touch-icon"
                    sizes="180x180"
                    href="/static/apple-touch-icon.png"
                >
                <meta name="apple-mobile-web-app-title" content="WanShiTong">
                <link rel="manifest" href="/static/site.webmanifest">
                // `theme.ts` fills this in to match the page background.
                <meta name="theme-color">

                <title>"Wan Shi Tong"</title>

                <link rel="stylesheet" href=(STYLESHEET_PATH)>
                // Not deferred: it sets the color theme before the page is painted.
                <script src=(THEME_SCRIPT_PATH)></script>
                // topcoat's client runtime, for signals and live regions. A
                // module, so it runs once the page is parsed.
                <script type="module" src="/static/topcoat-runtime@0.9.0.js"></script>

                <script defer="" src=(DIALOG_SCRIPT_PATH)></script>
                // Alpine components and plugins must load before Alpine itself.
                <script defer="" src=(COPY_SCRIPT_PATH)></script>
                <script defer="" src=(POSTERS_SCRIPT_PATH)></script>
                <script defer="" src=(SELECTION_SCRIPT_PATH)></script>
                <script defer="" src=(TREE_SCRIPT_PATH)></script>
                <script defer="" src=(GROUPS_SCRIPT_PATH)></script>
                <script defer="" src="/static/alpine-ajax@0.12.7.min.js"></script>
                <script defer="" src="/static/alpinejs-intersect@3.17.4.min.js"></script>
                <script defer="" src="/static/alpinejs-morph@3.17.4.min.js"></script>
                <script defer="" src="/static/alpinejs@3.17.4.min.js"></script>
            </head>
            <body class="group/page min-h-screen">
                <header
                    class="sticky top-0 z-10 border-b border-border bg-background/90 backdrop-blur"
                >
                    // Wider than the page, so that the row of links fits on wide screens.
                    <nav
                        id="site-nav"
                        class="mx-auto flex max-w-6xl items-center gap-2 px-6 py-2"
                    >
                        // A row of links, except in the compact header (see `compact`
                        // in styles.css).
                        <div data-nav-row="" class="flex shrink-0 gap-1 compact:hidden">
                            for (label, url, current) in &nav {
                                <a
                                    href=(url.as_str())
                                    aria-current=(*current)
                                    class=(button_variants(
                                        if current.is_some() {
                                            ButtonVariant::Secondary
                                        } else {
                                            ButtonVariant::Ghost
                                        },
                                        ButtonSize::Sm,
                                    ))
                                >
                                    (*label)
                                </a>
                            }
                            api_docs_link(
                                attrs: attributes! {
                                    class=(button_variants(ButtonVariant::Ghost, ButtonSize::Sm))
                                }
                            )
                        </div>

                        // The compact header: the current page's name, and a menu of
                        // the links instead of the row.
                        <span
                            class="hidden font-semibold compact:inline"
                            translate=(current_label.is_none().then_some("no"))
                        >
                            (current_label.unwrap_or("Wan Shi Tong"))
                        </span>
                        page_help::nav_page_help()

                        <div class="ml-auto flex items-center gap-1">
                            theme_picker()

                            <div class="hidden compact:block">
                                dropdown_menu(
                                    // A <details> only closes from its own button; also close it
                                    // on a tap outside the menu, or on Escape.
                                    attrs: attributes! {
                                        x-data=""
                                        x-on:click.outside="$el.open = false"
                                        x-on:keydown.escape.window="$el.open = false"
                                    },
                                    dropdown_menu_trigger(
                                        attrs: attributes! {
                                            class=(button_variants(ButtonVariant::Ghost, ButtonSize::Sm))
                                            aria-label="Menu"
                                        },
                                        lucide(icon: MENU, attrs: attributes! { class="size-5" })
                                    )
                                    dropdown_menu_content(
                                        attrs: attributes! { class="right-0 left-auto w-48" },
                                        for (label, url, current) in &nav {
                                            <a
                                                href=(url.as_str())
                                                aria-current=(*current)
                                                class=(if current.is_some() {
                                                    NAV_ITEM_CURRENT
                                                } else {
                                                    NAV_ITEM
                                                })
                                            >
                                                (*label)
                                            </a>
                                        }
                                        api_docs_link(
                                            attrs: attributes! { class=(format!("{NAV_ITEM} gap-1")) }
                                        )
                                    )
                                )
                            </div>
                        </div>
                    </nav>
                    // Not deferred: it makes the header compact, if the links don't
                    // fit, before it's painted.
                    <script src=(NAV_SCRIPT_PATH)></script>
                    draft::draft_bar()
                </header>

                <main class="mx-auto flex max-w-5xl flex-col gap-6 px-6 py-8">
                    error_boundary(
                        fallback: |error| {
                            let error = PageError::classify(error)?;
                            Ok(
                                view! {
                                    (error.status())
                                    <h1 class=(H1)>
                                        (error.status().canonical_reason().unwrap_or("Error"))
                                    </h1>
                                    <p class="text-muted-foreground">(error.message())</p>
                                },
                            )
                        },
                        (slot)
                    )
                </main>

                onboarding::onboarding_card()
            </body>
        </html>
    })
}

/// `aria-current` for a navigation link to `url`, if the page at `path` is its
/// page ("page") or one in its section ("true"): below its URL, or at or below
/// one of `also`.
fn aria_current(path: &str, url: &str, also: &[&str]) -> Option<&'static str> {
    // At `base` or below it, but not at a longer name, like `/files-old`.
    let under = |base: &str| {
        path.strip_prefix(base)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    };
    if path == url {
        Some("page")
    } else if under(url) || also.iter().any(|&base| under(base)) {
        Some("true")
    } else {
        None
    }
}

/// The header's link to the API reference, which opens in a new tab: it's a
/// page of its own, not one of the app's.
#[component]
async fn api_docs_link(attrs: Attributes) -> Result<impl View> {
    Ok(view! {
        <a href=(api_docs::DOCS_PATH) target="_blank" rel="noopener" (attrs)>
            "API"
            lucide(
                icon: ARROW_UP_RIGHT,
                attrs: attributes! { class="size-3.5 text-muted-foreground" }
            )
            <span class="sr-only">" (opens in a new tab)"</span>
        </a>
    })
}

/// The header's color theme menu: System, Light or Dark. `ts/theme.ts` applies
/// the choice and remembers it in this browser.
#[component]
async fn theme_picker() -> Result<impl View> {
    Ok(view! {
        dropdown_menu(
            attrs: attributes! {
                // Defined in ts/theme.ts.
                x-data="themePicker"
                x-on:click.outside="$el.open = false"
                x-on:keydown.escape.window="$el.open = false"
            },
            dropdown_menu_trigger(
                attrs: attributes! {
                    class=(button_variants(ButtonVariant::Ghost, ButtonSize::Sm))
                    aria-label="Theme"
                },
                // The current theme's icon. Until Alpine starts, the system one.
                lucide(
                    icon: MONITOR,
                    attrs: attributes! { class="size-5" x-show="theme === 'system'" }
                )
                lucide(
                    icon: SUN,
                    attrs: attributes! {
                        class="size-5"
                        x-show="theme === 'light'"
                        style="display: none"
                    }
                )
                lucide(
                    icon: MOON,
                    attrs: attributes! {
                        class="size-5"
                        x-show="theme === 'dark'"
                        style="display: none"
                    }
                )
            )
            dropdown_menu_content(
                attrs: attributes! { class="right-0 left-auto w-40" },
                for (theme, label, icon) in [
                    ("system", "System", MONITOR),
                    ("light", "Light", SUN),
                    ("dark", "Dark", MOON),
                ] {
                    dropdown_menu_item(
                        attrs: attributes! { type="button" x-on:click=(format!("pick('{theme}')")) },
                        lucide(icon: icon, attrs: attributes! { class="size-4" })
                        (label)
                        lucide(
                            icon: CHECK,
                            attrs: attributes! {
                                class="ml-auto size-4"
                                x-show=(format!("theme === '{theme}'"))
                            }
                        )
                    )
                }
            )
        )
    })
}

/// Until the guide is started or skipped, the welcome. After, the catalog, with
/// any query passed along, so bookmarks from when the catalog was here work.
#[page]
async fn home(cx: &Cx) -> Result<impl View> {
    let step = get_onboarding(app(cx)).await?.current_step();
    if step != Some(OnboardingStep::Welcome) {
        let mut catalog = href!(catalog::page).resolve(cx);
        if let Some(query) = uri(cx).query() {
            catalog = format!("{catalog}?{query}");
        }
        return Err(see_other(catalog).into());
    }
    Ok(view! { onboarding::welcome() })
}

// Unmatched URLs render the layout's 404 page instead of a bare response.
not_found!();

#[cfg(test)]
mod tests {
    use super::aria_current;

    #[test]
    fn a_link_is_current_on_its_page_and_in_its_section() {
        let catalog_also = ["/movies", "/tv-series"];
        assert_eq!(aria_current("/files", "/files", &[]), Some("page"));
        assert_eq!(aria_current("/files/1/a.mkv", "/files", &[]), Some("true"));
        assert_eq!(
            aria_current("/movies/1", "/catalog", &catalog_also),
            Some("true")
        );
        assert_eq!(
            aria_current("/tv-series/1", "/catalog", &catalog_also),
            Some("true")
        );
        assert_eq!(aria_current("/files-old", "/files", &[]), None);
        assert_eq!(aria_current("/draft", "/catalog", &catalog_also), None);
        assert_eq!(
            aria_current("/draft", "/corrections", &["/draft"]),
            Some("true")
        );
        assert_eq!(aria_current("/drafts", "/corrections", &["/draft"]), None);
        assert_eq!(
            aria_current("/assertions", "/corrections", &["/draft", "/assertions"]),
            Some("true")
        );
    }
}

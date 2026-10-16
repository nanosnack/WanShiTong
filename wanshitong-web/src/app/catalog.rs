use super::corrections;
use super::movies::movie_id::{self, MovieId};
use super::tv_series::series_id::{self, SeriesId};
use crate::app::page_help::page_help;
use crate::icons::{CHECK, IMAGE, lucide};
use crate::{
    context::app,
    cursor::{decode_cursor, encode_cursor},
    styles::{LINK, SECTION_H1, SECTION_HEADING_ROW},
    ui::{
        button::{ButtonSize, ButtonVariant, button_variants},
        checkbox::checkbox,
        input::input,
    },
};
use serde::{Deserialize, Serialize};
use topcoat::{
    Result,
    context::Cx,
    router::{href, page, query_params},
    view::{View, attributes, class, component, view},
};
use wanshitong_app::{
    domain::{MovieSummary, TvSeriesSummary},
    queries::{self, MovieSearchQuery, MovieSummaryList, TvSeriesSearchQuery, TvSeriesSummaryList},
};

#[query_params(error = bad_request)]
struct Search {
    kind: Option<Kind>,
    title: Option<String>,
    /// Only movies with extras. The checkbox sends "true" when checked and
    /// nothing otherwise.
    extras: Option<bool>,
    cursor: Option<String>,
}

/// Which kind of work the page lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    #[default]
    Movies,
    Tv,
}

/// Whether the page shows every movie or only those with extras.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExtrasFilter {
    All,
    OnlyWithExtras,
}

/// What the page is showing: the kind of work and the search.
#[derive(Clone, Debug)]
struct Showing {
    kind: Kind,
    title: Option<String>,
    /// Always `All` except for movies; nothing else has extras.
    extras: ExtrasFilter,
}

/// This page's URL query.
#[derive(Serialize)]
struct PageQuery<'a> {
    #[serde(skip_serializing_if = "is_movies")]
    kind: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    extras: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
}

fn is_movies(kind: &Kind) -> bool {
    *kind == Kind::Movies
}

impl<'a> PageQuery<'a> {
    fn new(showing: &'a Showing) -> Self {
        Self {
            kind: showing.kind,
            title: showing.title.as_deref(),
            extras: showing.extras == ExtrasFilter::OnlyWithExtras,
            cursor: None,
        }
    }
}

/// One page of the catalog's movies or TV series.
enum Works {
    Movies(Vec<MovieSummary>),
    TvSeries(Vec<TvSeriesSummary>),
}

/// The catalog's movies or TV series matching a title search, 200 at a time, as
/// a grid of posters. The kind switch picks which. The catalog holds only
/// identified works; the videos in none of them are on the files page. A
/// checkbox narrows movies to those with extras.
///
/// The search form replaces the whole grid as you type or tick, and pushes the
/// search URL. The last item loads the next page when it scrolls into view,
/// appending its posters to the grid. Both requests are for this page with
/// different parameters; Alpine AJAX takes the part it needs from the response.
///
/// The poster size is only a CSS variable, set by a slider, so it needs no
/// request.
#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let Search {
        kind,
        title,
        extras,
        cursor,
    } = query_params::<Search>(cx)?;
    let kind = kind.unwrap_or_default();
    let extras = match (kind, extras) {
        (Kind::Movies, Some(true)) => ExtrasFilter::OnlyWithExtras,
        _ => ExtrasFilter::All,
    };
    let showing = Showing {
        kind,
        title: title.clone(),
        extras,
    };

    let (works, next_cursor) = match kind {
        Kind::Movies => {
            let MovieSummaryList {
                movies,
                next_cursor,
            } = queries::list_movies(
                app(cx),
                &MovieSearchQuery {
                    title: title.clone(),
                    with_extras: extras == ExtrasFilter::OnlyWithExtras,
                    cursor: cursor.as_deref().and_then(decode_cursor),
                    limit: 200,
                },
            )
            .await?;
            (
                Works::Movies(movies),
                next_cursor.map(|c| encode_cursor(&c)),
            )
        }
        Kind::Tv => {
            let TvSeriesSummaryList {
                series,
                next_cursor,
            } = queries::list_tv_series(
                app(cx),
                &TvSeriesSearchQuery {
                    name: title.clone(),
                    cursor: cursor.as_deref().and_then(decode_cursor),
                    limit: 200,
                },
            )
            .await?;
            (
                Works::TvSeries(series),
                next_cursor.map(|c| encode_cursor(&c)),
            )
        }
    };

    let first_page = cursor.is_none();
    let here = href!(page).query(PageQuery::new(&showing)).resolve(cx);
    let next_page = next_cursor.map(|cursor| {
        href!(page)
            .query(PageQuery {
                cursor: Some(cursor),
                ..PageQuery::new(&showing)
            })
            .resolve(cx)
    });

    Ok(view! {
        <div class=(SECTION_HEADING_ROW)>
            <h1 class=(SECTION_H1)>"Catalog"</h1>
            page_help(
                section: true,
                <p>
                    "The movies and TV series found in your library. "
                    "The catalog always shows the canonical interpretation, "
                    "which changes as new files are ingested or new assertions are made. "
                    "If you see any mistakes, you can select and mark those works. "
                    "Marked works end up on the "
                    <a class=(LINK) href=(href!(corrections::page))>
                        "Corrections page"
                    </a>
                    "."
                </p>
            )
        </div>

        // Defined in ts/posters.ts. Sets how many posters the grid fits in a row.
        <div
            class="flex flex-col gap-6"
            x-data="posterSize"
            x-bind:style="{ '--poster-columns': shown }"
        >
            kind_switch(showing: showing.clone())

            <div class="flex flex-wrap items-center gap-x-4 gap-y-2">
                <form
                    class="flex min-w-48 flex-1 flex-wrap items-center gap-x-4 gap-y-2"
                    method="get"
                    action=(href!(page))
                    x-data=""
                    x-target.push.nofocus="catalog-list kind-switch"
                    x-on:input.debounce.300ms="$el.requestSubmit()"
                >
                    input(
                        attrs: attributes! {
                            type="search"
                            name="title"
                            placeholder=(match kind {
                                Kind::Movies => "Search movies…",
                                Kind::Tv => "Search TV series…",
                            })
                            value=(title.as_deref().unwrap_or_default())
                            class="min-w-48 flex-1"
                        }
                    )
                    match kind {
                        Kind::Movies => {
                            <label class="flex items-center gap-2 text-sm">
                                checkbox(
                                    attrs: attributes! {
                                        name="extras"
                                        value="true"
                                        checked=(extras == ExtrasFilter::OnlyWithExtras)
                                    }
                                )
                                "Only with extras"
                            </label>
                        }
                        Kind::Tv => {
                            <input type="hidden" name="kind" value="tv">
                        }
                    }
                </form>

                <label
                    class="flex items-center gap-2 text-muted-foreground"
                    title="Poster size"
                >
                    lucide(icon: IMAGE, attrs: attributes! { class="size-4" })
                    // From 10 posters to a row, or as many as fit, up to 2, bigger to
                    // the right: the value is the count negated. See ts/posters.ts.
                    <input
                        type="range"
                        min="-10"
                        max="-2"
                        value="-4"
                        x-bind:min="-fitting"
                        aria-label="Poster size"
                        x-bind:aria-valuetext="shown + ' per row'"
                        class="w-28 accent-primary"
                        x-model.number="size"
                    >
                    lucide(icon: IMAGE, attrs: attributes! { class="size-5" })
                </label>
            </div>

            <div id="catalog-list" x-data="">
                poster_grid(
                    posters: posters(cx, works),
                    first_page: first_page,
                    next_page: next_page,
                    return_to: here.clone()
                )
            </div>
        </div>
    })
}

/// Links to this page's search of each kind of work. The current one is
/// highlighted. Movie-only filters are dropped when switching to TV series.
#[component]
async fn kind_switch(showing: Showing) -> Result<impl View> {
    Ok(view! {
        <nav id="kind-switch" class="flex gap-1" aria-label="Kind of work">
            for (option, label) in [(Kind::Movies, "Movies"), (Kind::Tv, "TV Series")] {
                <a
                    href=(href!(page).query(
                        PageQuery {
                            kind: option,
                            extras: option == Kind::Movies
                                && showing.extras == ExtrasFilter::OnlyWithExtras,
                            ..PageQuery::new(&showing)
                        },
                    ))
                    aria-current=((option == showing.kind).then_some("page"))
                    class=(button_variants(
                        if option == showing.kind {
                            ButtonVariant::Secondary
                        } else {
                            ButtonVariant::Ghost
                        },
                        ButtonSize::Sm,
                    ))
                >
                    (label)
                </a>
            }
        </nav>
    })
}

/// A work in the poster grid.
struct Poster {
    /// What selecting it marks, e.g. `movie:UUID`; see `corrections::mark_many`.
    work: String,
    href: String,
    title: String,
    /// Shown under the title, e.g. the release year.
    subtitle: Option<String>,
    /// TMDB's path for the poster image, if there is one.
    tmdb_path: Option<String>,
}

fn posters(cx: &Cx, works: Works) -> Vec<Poster> {
    match works {
        Works::Movies(movies) => movies.into_iter().map(|m| movie_poster(cx, m)).collect(),
        Works::TvSeries(series) => series.into_iter().map(|s| series_poster(cx, s)).collect(),
    }
}

fn movie_poster(cx: &Cx, movie: MovieSummary) -> Poster {
    Poster {
        work: format!("movie:{}", movie.uuid.0),
        href: href!(movie_id::page, MovieId(movie.uuid.0)).resolve(cx),
        title: movie.title,
        subtitle: movie.release_year.map(|year| year.to_string()),
        tmdb_path: movie.tmdb_poster_path,
    }
}

fn series_poster(cx: &Cx, series: TvSeriesSummary) -> Poster {
    Poster {
        work: format!("series:{}", series.uuid.0),
        href: href!(series_id::page, SeriesId(series.uuid.0)).resolve(cx),
        subtitle: Some(match series.episode_count {
            1 => "1 episode".to_string(),
            n => format!("{n} episodes"),
        }),
        title: series.name,
        tmdb_path: series.tmdb_poster_path,
    }
}

/// Posters with their titles, `--poster-columns` to a row (see `poster-grid` in
/// styles.css), each with a circle to select it. While any are selected, a
/// bar at the bottom of the page marks them as needing correction, together.
#[component]
async fn poster_grid(
    posters: Vec<Poster>,
    first_page: bool,
    next_page: Option<String>,
    /// This page, to come back to after marking.
    return_to: String,
) -> Result<impl View> {
    Ok(view! {
        // Defined in ts/selection.ts, which says how posters are selected.
        <div
            class="group/grid"
            x-data="selection"
            x-bind:class="selected > 0 && 'selecting'"
            x-on:change="count()"
            x-on:pointerdown="pointerDown($event)"
            x-on:click="click($event)"
            x-on:contextmenu="contextMenu($event)"
        >
            <ul id="catalog-results" x-merge="append" class="poster-grid">
                if posters.is_empty() && first_page {
                    <li class="col-span-full text-sm text-muted-foreground">
                        "Nothing matches"
                    </li>
                }

                for poster in &posters {
                    <li class="group/poster relative select-none">
                        // Beside the link, so that selecting doesn't open the work.
                        // Hidden until hovered with a mouse, or anything is selected. On a
                        // touch screen, it can't be tapped until then: a long press selects.
                        <label
                            class=(class!(
                                "absolute top-1.5 left-1.5 z-[1] cursor-pointer",
                                "opacity-0 transition-opacity has-focus-visible:opacity-100",
                                "pointer-fine:group-hover/poster:opacity-100",
                                "group-[.selecting]/grid:opacity-100",
                                "pointer-coarse:pointer-events-none",
                                "pointer-coarse:group-[.selecting]/grid:pointer-events-auto",
                            ))
                            title="Select"
                        >
                            <input
                                type="checkbox"
                                name="work"
                                value=(poster.work.as_str())
                                form="mark-selected"
                                aria-label=(format!("Select {}", poster.title))
                                class="peer sr-only"
                            >
                            <span
                                class=(class!(
                                    "flex size-6 items-center justify-center rounded-full",
                                    "border-2 border-white bg-black/30 text-primary-foreground shadow",
                                    "peer-checked:border-primary peer-checked:bg-primary",
                                    "peer-focus-visible:ring-2 peer-focus-visible:ring-ring",
                                    "[&>svg]:hidden peer-checked:[&>svg]:block",
                                ))
                            >
                                lucide(icon: CHECK, attrs: attributes! { class="size-4" })
                            </span>
                        </label>
                        <a
                            class="group flex flex-col gap-2 [-webkit-touch-callout:none]"
                            href=(poster.href.as_str())
                        >
                            // Ringed while selected.
                            <div
                                class=(class!(
                                    "aspect-[2/3] overflow-hidden rounded-md bg-foreground/5",
                                    "shadow-sm ring-sky-400 ring-offset-2 ring-offset-background",
                                    "group-has-[:checked]/poster:ring-3",
                                ))
                            >
                                // TODO: cache posters on the backend
                                if let Some(path) = &poster.tmdb_path {
                                    // The title is beside it, so the image needs no alt text.
                                    <img
                                        class=(class!(
                                            "size-full object-cover transition-opacity",
                                            "group-hover:opacity-90",
                                        ))
                                        src=(format!("https://image.tmdb.org/t/p/w342{path}"))
                                        srcset=(poster_srcset(path))
                                        sizes="auto, 240px"
                                        loading="lazy"
                                        decoding="async"
                                        alt=""
                                    > // The poster's rendered width, where the browser supports
                                    // `auto` (it needs lazy loading); elsewhere, a medium size.
                                } else {
                                    <div
                                        class=(class!(
                                            "flex size-full items-center justify-center p-2",
                                            "text-center text-xs text-muted-foreground",
                                        ))
                                    >
                                        "No poster"
                                    </div>
                                }
                            </div>
                            <div class="flex flex-col text-sm">
                                <span
                                    class=(class!(
                                        "line-clamp-2 font-medium underline-offset-4",
                                        "group-hover:underline pointer-coarse:underline",
                                    ))
                                    translate="no"
                                >
                                    (poster.title.as_str())
                                </span>
                                if let Some(subtitle) = &poster.subtitle {
                                    <span class="text-xs text-muted-foreground">
                                        (subtitle.as_str())
                                    </span>
                                }
                            </div>
                        </a>
                    </li>
                }

                if let Some(next_page) = next_page {
                    <li
                        class="col-span-full text-sm text-muted-foreground"
                        data-next=(next_page)
                        data-target="catalog-results"
                        x-intersect.once="more($el)"
                    >
                        "Loading more…"
                    </li>
                }
            </ul>

            corrections::marking::mark_selected_bar(return_to: return_to)
        </div>
    })
}

/// TMDB's poster sizes, for the browser to pick from. `path` starts with a slash.
fn poster_srcset(path: &str) -> String {
    [154, 185, 342, 500, 780]
        .map(|width| format!("https://image.tmdb.org/t/p/w{width}{path} {width}w"))
        .join(", ")
}

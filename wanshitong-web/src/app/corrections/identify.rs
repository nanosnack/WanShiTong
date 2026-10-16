//! The Identify panel: a knowledge base search, and the form that identifies
//! the selected files as part of the chosen work. Beside the cards on the
//! Corrections page, so its routes are under `/corrections`.

// A group: no URL segment of its own.
topcoat::router::segment!(kind = Group);

use crate::app::file_key::FileKey;
use crate::{
    app::tmdb_id::{TmdbKind, parse_tmdb_ref},
    context::app,
    ui::{
        button::{ButtonSize, ButtonVariant, button},
        field::{field, field_label},
        input::input,
    },
};
use topcoat::{
    Result,
    context::Cx,
    router::{
        content::RawForm,
        error::{SeeOther, bad_request, see_other},
        href, route,
    },
    view::{View, attributes, class, component, view},
};
use wanshitong_app::domain::PathId;
use wanshitong_app::{
    AppErrorContext,
    domain::{ClaimPredicate, ExternalId, FileRole},
    queries::{self, KbSearchHit, KbWorkKind},
};

/// A knowledge base search's results, or why there are none.
pub(super) enum SearchResults {
    Found(Vec<SearchHit>),
    Unavailable(String),
}

pub(super) struct SearchHit {
    kind: TmdbKind,
    hit: KbSearchHit,
    /// A movie's runtime in minutes, to compare with the files' durations.
    runtime_minutes: Option<u32>,
}

/// How many search results are looked up for their runtimes.
const RUNTIMES_LOOKED_UP: usize = 6;

pub(super) async fn search(
    cx: &Cx,
    kind: TmdbKind,
    query: &str,
    year: Option<u32>,
) -> Result<SearchResults> {
    let kb_kind = match kind {
        TmdbKind::Movie => KbWorkKind::Movie,
        TmdbKind::Tv => KbWorkKind::TvSeries,
    };
    let hits = match queries::search_knowledge_base(app(cx), kb_kind, query, year).await {
        Ok(hits) => hits,
        Err(error)
            if matches!(
                error.context(),
                AppErrorContext::NoKnowledgeBase | AppErrorContext::KnowledgeBase(_)
            ) =>
        {
            return Ok(SearchResults::Unavailable(format!("{error:#}")));
        }
        Err(error) => return Err(error.into()),
    };
    let mut results = Vec::with_capacity(hits.len());
    for (i, hit) in hits.into_iter().enumerate() {
        let runtime_minutes = match kind {
            TmdbKind::Movie if i < RUNTIMES_LOOKED_UP => {
                queries::get_kb_movie(app(cx), hit.tmdb_id)
                    .await
                    .ok()
                    .flatten()
                    .and_then(|movie| movie.runtime_minutes)
            }
            _ => None,
        };
        results.push(SearchHit {
            kind,
            hit,
            runtime_minutes,
        });
    }
    Ok(SearchResults::Found(results))
}

/// Finds a work in the knowledge base, and identifies the selected files as
/// part of it. The search swaps only its results into the page, so that the
/// selection stays.
#[component]
pub(super) async fn identify_panel(
    query: &str,
    kind: TmdbKind,
    year: Option<u32>,
    results: Option<&SearchResults>,
) -> Result<impl View> {
    Ok(view! {
        <aside
            class=(class!(
                "flex flex-col gap-3 self-start rounded-lg border border-border p-4",
                "lg:sticky lg:top-16",
            ))
        >
            <h2 class="font-semibold">"Identify"</h2>
            <form
                class="flex flex-col gap-2"
                method="get"
                action=(href!(super::page))
                x-data=""
                x-target.nofocus="search-results"
            >
                input(
                    attrs: attributes! {
                        type="search"
                        name="query"
                        placeholder="Search TMDB…"
                        value=(query)
                    }
                )
                <div class="flex gap-2">
                    <select
                        name="kind"
                        class="rounded-md border border-border bg-background px-2 text-sm"
                    >
                        <option value="movie" selected=(kind == TmdbKind::Movie)>
                            "Movies"
                        </option>
                        <option value="tv" selected=(kind == TmdbKind::Tv)>
                            "TV series"
                        </option>
                    </select>
                    input(
                        attrs: attributes! {
                            type="number"
                            name="year"
                            placeholder="Year"
                            value=(year.map(|year| year.to_string()).unwrap_or_default())
                            class="w-24"
                        }
                    )
                    button(
                        variant: ButtonVariant::Secondary,
                        size: ButtonSize::Sm,
                        attrs: attributes! { type="submit" },
                        "Search"
                    )
                </div>
            </form>

            <form
                id="identify"
                class="flex flex-col gap-3"
                method="post"
                action=(href!(identify))
            >
                // Scrolls rather than growing the panel; the search swaps it whole.
                <div
                    id="search-results"
                    class=(class!(
                        "flex max-h-80 flex-col gap-1 overflow-y-auto overscroll-contain",
                        "pr-1 text-sm",
                    ))
                >
                    match results {
                        None => {

                        }
                        Some(SearchResults::Unavailable(reason)) => {
                            <p class="text-muted-foreground">(reason.as_str())</p>
                        }
                        Some(SearchResults::Found(hits)) => {
                            if hits.is_empty() {
                                <p class="text-muted-foreground">"No results."</p>
                            }
                            for result in hits {
                                search_hit(result: result)
                            }
                        }
                    }
                </div>

                field(
                    field_label(
                        attrs: attributes! { for="identify-tmdb" },
                        "Or a TMDB ID or URL"
                    )
                    input(
                        attrs: attributes! {
                            id="identify-tmdb"
                            name="tmdb"
                            placeholder="e.g. 80809, or a /tv/ URL"
                        }
                    )
                )

                <fieldset class="flex flex-col gap-1 text-sm">
                    <legend class="mb-1 font-medium">"As"</legend>
                    for (value, label) in [
                        ("main", "The movie's main file"),
                        ("extra", "An extra, of a movie or a series"),
                        ("special", "A special of the series, e.g. an OVA"),
                        ("episode", "Episodes of the series, numbered in order"),
                    ] {
                        <label class="flex items-center gap-2">
                            <input
                                type="radio"
                                name="part"
                                value=(value)
                                checked=(value == "main")
                            >
                            (label)
                        </label>
                    }
                    <div class="ml-6 flex gap-2">
                        input(
                            attrs: attributes! {
                                type="number"
                                name="season"
                                placeholder="Season"
                                min="0"
                                class="w-24"
                            }
                        )
                        input(
                            attrs: attributes! {
                                type="number"
                                name="episode"
                                placeholder="First episode"
                                min="1"
                                class="w-32"
                            }
                        )
                    </div>
                </fieldset>

                button(attrs: attributes! { type="submit" }, "Identify selected")
            </form>
        </aside>
    })
}

#[component]
async fn search_hit(result: &SearchHit) -> Result<impl View> {
    let SearchHit {
        kind,
        hit,
        runtime_minutes,
    } = result;
    let (value, url_kind) = match kind {
        TmdbKind::Movie => (format!("movie:{}", hit.tmdb_id), "movie"),
        TmdbKind::Tv => (format!("tv:{}", hit.tmdb_id), "tv"),
    };
    let year = hit
        .year
        .map(|year| format!(" ({year})"))
        .unwrap_or_default();
    let runtime = runtime_minutes
        .map(|minutes| format!(", {minutes}m"))
        .unwrap_or_default();
    Ok(view! {
        <label class="flex items-start gap-2 rounded-md p-1 hover:bg-foreground/5">
            <input class="mt-1" type="radio" name="target" value=(value.as_str())>
            <span class="flex min-w-0 flex-col">
                <span>
                    <span translate="no">(hit.title.as_str())</span>
                    (format!("{year}{runtime}"))
                    " "
                    <a
                        class="text-muted-foreground underline"
                        href=(format!(
                            "https://www.themoviedb.org/{url_kind}/{}",
                            hit.tmdb_id,
                        ))
                        target="_blank"
                        translate="no"
                    >
                        (format!("tmdb:{}", hit.tmdb_id))
                    </a>
                </span>
                if !hit.overview.is_empty() {
                    <span class="line-clamp-2 text-muted-foreground">
                        (hit.overview.as_str())
                    </span>
                }
            </span>
        </label>
    })
}

/// The identify form: the selected files, the work they're of, chosen from
/// the search results or typed as a TMDB ID or URL, and what they are of it.
#[derive(Default)]
struct IdentifyForm {
    files: Vec<PathId>,
    /// The search result chosen, `movie:ID` or `tv:ID`.
    target: Option<String>,
    /// A TMDB ID or URL typed in.
    tmdb: String,
    /// What the files are of the work, or None if the field names no part.
    part: Option<Part>,
    season: Option<u32>,
    /// The first episode's number.
    episode: Option<u32>,
}

/// The role the files play in a work, as named by the identify form's `part`
/// field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Main,
    Extra,
    Special,
    Episode,
}

impl Part {
    fn from_field(value: &str) -> Option<Self> {
        match value {
            "main" => Some(Self::Main),
            "extra" => Some(Self::Extra),
            "special" => Some(Self::Special),
            "episode" => Some(Self::Episode),
            _ => None,
        }
    }
}

impl IdentifyForm {
    /// The form as posted. Its `file` field repeats, which a typed form
    /// can't take.
    fn parse(body: &[u8]) -> Result<Self> {
        let mut form = Self::default();
        for (key, value) in form_urlencoded::parse(body) {
            match &*key {
                "file" => {
                    let FileKey { data_source, path } = FileKey::from_value(&value)
                        .ok_or_else(|| bad_request("a file is DATA_SOURCE|PATH"))?;
                    form.files.push(PathId::new(data_source, path));
                }
                "target" => form.target = Some(value.into_owned()),
                "tmdb" => form.tmdb = value.trim().to_string(),
                "part" => form.part = Part::from_field(&value),
                "season" if !value.is_empty() => {
                    form.season = Some(value.parse().map_err(|_| bad_request("not a season"))?)
                }
                "episode" if !value.is_empty() => {
                    form.episode = Some(value.parse().map_err(|_| bad_request("not an episode"))?)
                }
                _ => {}
            }
        }
        Ok(form)
    }

    /// The work the files are of: the TMDB ID typed, which wins over a search
    /// result, as it's typed on purpose, or else the search result chosen.
    fn work(&self) -> Option<(TmdbKind, u32)> {
        if !self.tmdb.is_empty() {
            // A typed ID that doesn't say its kind is of the kind the part implies.
            let kind = match self.part {
                Some(Part::Special | Part::Episode) => TmdbKind::Tv,
                _ => TmdbKind::Movie,
            };
            return parse_tmdb_ref(&self.tmdb, kind);
        }
        match self.target.as_deref()?.split_once(':')? {
            ("movie", id) => Some((TmdbKind::Movie, id.parse().ok()?)),
            ("tv", id) => Some((TmdbKind::Tv, id.parse().ok()?)),
            _ => None,
        }
    }
}

/// Identifies the selected files as part of the chosen work, in the draft,
/// then shows the corrections again. Several files as episodes are numbered
/// in order of their paths, from the first episode given.
#[route(POST "./identify")]
async fn identify(cx: &Cx, RawForm(body): RawForm) -> Result<SeeOther> {
    let form = IdentifyForm::parse(&body)?;
    if form.files.is_empty() {
        return Err(bad_request("Select the files to identify.").into());
    }
    let Some((kind, id)) = form.work() else {
        return Err(bad_request("Choose a search result, or enter a TMDB ID or URL.").into());
    };
    let IdentifyForm {
        mut files,
        part,
        season,
        episode,
        ..
    } = form;
    let id = ExternalId::Tmdb(id);
    match (kind, part) {
        (TmdbKind::Movie, Some(Part::Main)) => {
            let claims = vec![
                ClaimPredicate::MovieExternalId(id),
                ClaimPredicate::FileRole(FileRole::Main),
            ];
            super::correct(cx, &files, claims).await?;
        }
        (TmdbKind::Movie, Some(Part::Extra)) => {
            let claims = vec![
                ClaimPredicate::MovieExternalId(id),
                ClaimPredicate::FileRole(FileRole::Extra),
            ];
            super::correct(cx, &files, claims).await?;
        }
        (TmdbKind::Tv, Some(Part::Special)) => {
            let claims = vec![
                ClaimPredicate::TvSeriesExternalId(id),
                ClaimPredicate::Special,
            ];
            super::correct(cx, &files, claims).await?;
        }
        (TmdbKind::Tv, Some(Part::Extra)) => {
            let claims = vec![
                ClaimPredicate::TvSeriesExternalId(id),
                ClaimPredicate::FileRole(FileRole::Extra),
            ];
            super::correct(cx, &files, claims).await?;
        }
        (TmdbKind::Tv, Some(Part::Episode)) => {
            let (Some(season), Some(first)) = (season, episode) else {
                return Err(bad_request("Give the season and the first episode.").into());
            };
            files.sort();
            for (number, file) in (first..).zip(&files) {
                let claims = vec![
                    ClaimPredicate::TvSeriesExternalId(id.clone()),
                    ClaimPredicate::Season(season),
                    ClaimPredicate::EpisodeRange([number, number]),
                ];
                super::correct(cx, std::slice::from_ref(file), claims).await?;
            }
        }
        (TmdbKind::Movie, _) => {
            return Err(bad_request("A movie's files are its main file or its extras.").into());
        }
        (TmdbKind::Tv, _) => {
            return Err(
                bad_request("A series' files are its episodes, specials or extras.").into(),
            );
        }
    }
    Ok(see_other(href!(super::page).resolve(cx)))
}

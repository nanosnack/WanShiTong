use crate::{
    app::{
        data_sources::data_source_id::{self, DataSourceId},
        draft, files,
        page_help::page_help,
    },
    context::app,
    styles::{H1, H2, LINK, MONO},
    ui::card::{card, card_content, card_header, card_title},
};
use std::collections::HashSet;
use topcoat::{
    Result,
    context::Cx,
    router::{error::RouterErrorExt, href, page, path_param},
    view::{View, class, component, view},
};
use uuid::Uuid;
use wanshitong_app::domain::PathId;
use wanshitong_app::{
    domain::{ClaimPredicate, DataSourceRef, TvSeriesDetails, TvSeriesFile, TvSeriesUuid},
    queries,
};

path_param!(pub series_id: Uuid, error = not_found);

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let id = TvSeriesUuid(*path_param::<SeriesId>(cx)?);
    let series = queries::get_tv_series(app(cx), id)
        .await?
        .ok_or_not_found()?;
    Ok(view! { series_page(series: series) })
}

/// The files to assert the series' identity about: its episodes', in season
/// and episode order, then its extras that aren't another file's. Those
/// follow the file they're an extra of, and an identity asserted about them
/// would outrank that. A file of several episodes is listed once.
pub(crate) fn asserted_files(series: &TvSeriesDetails) -> Vec<&TvSeriesFile> {
    let mut seen = HashSet::new();
    let is_extra_of_another = |file: &TvSeriesFile| {
        file.audit_log
            .claims
            .iter()
            .any(|claim| claim.active && matches!(claim.pred, ClaimPredicate::ExtraOf(_)))
    };
    series
        .seasons
        .iter()
        .flat_map(|season| &season.episodes)
        .flat_map(|episode| &episode.files)
        .chain(
            series
                .extras
                .iter()
                .filter(|file| !is_extra_of_another(file)),
        )
        .filter(|file| seen.insert((file.data_source_uuid, &file.rel_path)))
        .collect()
}

#[component]
async fn series_page(cx: &Cx, series: TvSeriesDetails) -> Result<impl View> {
    let file_paths: Vec<_> = asserted_files(&series)
        .into_iter()
        .map(|file| PathId::new(file.data_source_uuid, &file.rel_path))
        .collect();
    let episode_count: usize = series.seasons.iter().map(|s| s.episodes.len()).sum();
    // Episodes of an unknown season aren't a season of their own.
    let season_count = series.seasons.iter().filter(|s| s.number.is_some()).count();
    let has_files = episode_count > 0 || !series.extras.is_empty() || !series.discs.is_empty();
    let here = href!(page, SeriesId(series.uuid.0)).resolve(cx);

    Ok(view! {
        <header class="flex flex-col gap-4 sm:flex-row sm:items-start">
            // TODO: cache posters on the backend
            if let Some(path) = &series.tmdb_poster_path {
                <img
                    class="w-full max-w-xs shrink-0 rounded-lg shadow-sm sm:w-80"
                    src=(format!("https://image.tmdb.org/t/p/w780/{path}"))
                >
            } else {
                <div
                    class=(class!(
                        "flex aspect-[2/3] w-full max-w-xs shrink-0 items-center justify-center",
                        "rounded-lg bg-foreground/5 text-sm text-muted-foreground shadow-sm",
                        "sm:w-80",
                    ))
                >
                    "No poster"
                </div>
            }

            <div class="flex flex-col gap-2">
                <div class="flex items-center gap-2">
                    <h1 class=(H1) translate="no">(series.name.as_str())</h1>
                    page_help(
                        <p>
                            "A TV series in the catalog, with metadata from TMDB, "
                            "and all associated files. Open a file to see how it was "
                            "identified."
                        </p>
                        <p>
                            "If it's wrong, select its files and mark them, then "
                            "correct them on the Corrections page."
                        </p>
                    )
                </div>
                <p class="text-lg text-muted-foreground">
                    if let Some(year) = series.first_air_year {
                        (format!("{year} · "))
                    }
                    (seasons_and_episodes(
                        season_count,
                        episode_count,
                        series.extras.len(),
                    ))
                </p>
                if let Some(tagline) = &series.tagline {
                    <p class="italic text-muted-foreground">(tagline.as_str())</p>
                }
                if let Some(overview) = &series.overview {
                    <p class="max-w-prose text-sm">(overview.as_str())</p>
                }
            </div>
        </header>

        draft::pending_in_draft(files: file_paths)

        card(
            card_header(card_title("Details"))
            card_content(
                <dl class="kv-list">
                    if let Some(original_name) = &series.original_name {
                        <div>
                            <dt>"Original Name"</dt>
                            <dd translate="no">(original_name.as_str())</dd>
                        </div>
                    }

                    if let Some(original_language) = &series.original_language {
                        <div>
                            <dt>"Original Language"</dt>
                            <dd translate="no">(original_language.as_str())</dd>
                        </div>
                    }

                    <div>
                        <dt>"UUID"</dt>
                        <dd class=(MONO) translate="no">(series.uuid.to_string())</dd>
                    </div>

                    if let Some(id) = series.tmdb_id {
                        <div>
                            <dt>"TMDB ID"</dt>
                            <dd>
                                <a
                                    class=(LINK)
                                    href=(format!("https://www.themoviedb.org/tv/{id}"))
                                    translate="no"
                                >
                                    (id.to_string())
                                </a>
                            </dd>
                        </div>
                    }

                    if let Some(id) = &series.imdb_id {
                        <div>
                            <dt>"IMDB ID"</dt>
                            <dd>
                                <a
                                    class=(LINK)
                                    href=(format!("https://www.imdb.com/title/{id}"))
                                    translate="no"
                                >
                                    (id.to_string())
                                </a>
                            </dd>
                        </div>
                    }

                    if let Some(id) = series.tvdb_id {
                        <div>
                            <dt>"TVDB ID"</dt>
                            <dd>
                                <a
                                    class=(LINK)
                                    href=(format!("https://thetvdb.com/dereferrer/series/{id}"))
                                    translate="no"
                                >
                                    (id.to_string())
                                </a>
                            </dd>
                        </div>
                    }

                    // A series' files can be spread across data sources.
                    if !series.data_sources.is_empty() {
                        <div>
                            <dt>
                                (if series.data_sources.len() == 1 {
                                    "Data Source"
                                } else {
                                    "Data Sources"
                                })
                            </dt>
                            <dd class="flex flex-col">
                                for DataSourceRef { uuid, name } in &series.data_sources {
                                    <a
                                        class=(LINK)
                                        href=(href!(data_source_id::page, DataSourceId(uuid.0)))
                                        translate="no"
                                    >
                                        (name.as_str())
                                    </a>
                                }
                            </dd>
                        </div>
                    }
                </dl>
            )
        )

        <section class="flex flex-col gap-2">
            <h2 class=(H2)>"Files"</h2>
            if has_files {
                files::work_files(work: series.uuid.0, return_to: here)
            } else {
                <p class="text-muted-foreground">
                    "No files associated with this series."
                </p>
            }
        </section>
    })
}

/// E.g. `2 seasons, 24 episodes, 1 extra`. Seasons go unsaid when every
/// episode's season is unknown, e.g. `381 episodes`.
fn seasons_and_episodes(seasons: usize, episodes: usize, extras: usize) -> String {
    let mut parts = Vec::new();
    match seasons {
        0 if episodes > 0 => (),
        1 => parts.push("1 season".to_string()),
        n => parts.push(format!("{n} seasons")),
    }
    parts.push(match episodes {
        1 => "1 episode".to_string(),
        n => format!("{n} episodes"),
    });
    match extras {
        0 => (),
        1 => parts.push("1 extra".to_string()),
        n => parts.push(format!("{n} extras")),
    }
    parts.join(", ")
}

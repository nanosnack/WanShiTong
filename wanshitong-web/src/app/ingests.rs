pub mod ingest_id;

use crate::app::page_help::page_help;
use crate::{
    app::data_sources::{self, data_source_id},
    context::app,
    display,
    error::PageError,
    styles::{LINK, MONO, NUMBER, SECTION_H1, SECTION_HEADING_ROW},
    ui::{
        badge::{BadgeVariant, badge},
        progress::progress,
        table::{table, table_body, table_cell, table_head, table_header, table_row},
    },
};
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{
        content::Form,
        error::{RouterErrorExt, SeeOther, see_other},
        href, page, route,
    },
    runtime::{connected, shard},
    view::{View, attributes, component, emit, live, view},
};
use wanshitong_app::{
    commands,
    domain::DataSourceUuid,
    ingest::{IngestState, IngestStatus, IngestUuid},
    queries,
};

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let ingests = queries::list_ingests(app(cx)).await?;

    Ok(view! {
        <div class=(SECTION_HEADING_ROW)>
            <h1 class=(SECTION_H1)>"Ingests"</h1>
            page_help(
                section: true,
                <p>
                    "An ingest reads every file in a data source and records a snapshot of: "
                    "paths, sizes, and media file probes. "
                    "It's easiest to let WST watch your data sources for changes "
                    "and automatically trigger ingest, but you can also "
                    "start one manually from the Data Sources page."
                </p>
                <p>
                    "When an ingest finishes, an interpretation of the latest ingest of every "
                    "data source starts, and becomes canonical if it succeeds."
                </p>
            )
        </div>

        if ingests.is_empty() {
            <p class="text-muted-foreground">
                "There are no ingests. Try ingesting from a data source."
            </p>
        } else {
            table(
                attrs: attributes! { class="stacked-table" },
                table_header(
                    table_row(
                        table_head("Started")
                        table_head("Data Source")
                        table_head("Status")
                        table_head(
                            attrs: attributes! { class=(NUMBER) },
                            "Files Recorded"
                        )
                        table_head(attrs: attributes! { class=(NUMBER) }, "Elapsed")
                    )
                )
                table_body(
                    #[key(ingest.ingest.uuid.0.as_bytes())]
                    for ingest in &ingests {
                        table_row(
                            table_cell(
                                <a
                                    class=(format!("{LINK} {MONO}"))
                                    data-row-link=""
                                    translate="no"
                                    href=(href!(
                                        ingest_id::page,
                                        ingest_id::IngestId(ingest.ingest.uuid.0),
                                    ))
                                >
                                    (display::ts(&ingest.started_at()).to_string())
                                </a>
                            )
                            table_cell(
                                attrs: attributes! { data-label="Source" },
                                <a
                                    class=(LINK)
                                    href=(href!(
                                        data_source_id::page,
                                        data_source_id::DataSourceId(
                                            ingest.data_source.uuid.0,
                                        ),
                                    ))
                                    translate="no"
                                >
                                    (ingest.data_source.name.as_str())
                                </a>
                            )
                            table_cell(
                                attrs: attributes! { data-label="Status" },
                                if matches!(ingest.state(), IngestState::Pending) {
                                    live_ingest_state(
                                        ingest_id: ingest.ingest.uuid.0.to_string()
                                    )
                                } else {
                                    ingest_state(details: ingest)
                                }
                            )
                            table_cell(
                                attrs: attributes! { class=(NUMBER) data-label="Files" },
                                (ingest.files_recorded().to_string())
                            )
                            table_cell(
                                attrs: attributes! { class=(NUMBER) data-label="Elapsed" },
                                if matches!(ingest.state(), IngestState::Pending) {
                                    live_ingest_elapsed(
                                        ingest_id: ingest.ingest.uuid.0.to_string()
                                    )
                                } else {
                                    (display::human_duration(ingest.elapsed()))
                                }
                            )
                        )
                    }
                )
            )
        }
    })
}

#[derive(Deserialize)]
struct StartForm {
    data_source_id: DataSourceUuid,
}

/// Starts ingesting a data source, then shows the data sources, where it links
/// to the ingest's progress.
#[route(POST)]
async fn start(cx: &Cx, Form(form): Form<StartForm>) -> Result<SeeOther> {
    async {
        queries::get_data_source(app(cx), form.data_source_id)
            .await?
            .ok_or_not_found()?;
        Ok(commands::start_ingest(app(cx), form.data_source_id).await?)
    }
    .await
    .map_err(PageError::log)?;

    Ok(see_other(href!(data_sources::page).resolve(cx)))
}

/// How far an ingest has got: files recorded, or found unreadable, out of
/// files seen. Files are seen well before they're probed and recorded, so the
/// total settles early on.
#[component]
async fn ingest_progress_bar(details: &IngestStatus) -> Result<impl View> {
    let recorded = details.files_recorded() + details.files_failed();
    let seen = details.files_seen();
    Ok(view! {
        <div class="flex items-center gap-2">
            progress(
                value: recorded,
                max: seen,
                attrs: attributes! { class="min-w-24" aria-label="Files recorded" }
            )
            <span class="text-xs whitespace-nowrap text-muted-foreground tabular-nums">
                (format!("{recorded} / {seen}"))
            </span>
        </div>
    })
}

/// A running ingest's progress bar, live until the ingest stops, when it
/// becomes the ingest's status.
///
/// The shard has its own endpoint, so `ingest_id` comes from the browser and is
/// checked like any other request input. It is a string because shard arguments
/// must be types the browser runtime can represent, and `Uuid` isn't one.
#[shard]
async fn live_ingest_state(cx: &Cx, ingest_id: String) -> Result<impl View> {
    let id = IngestUuid(ingest_id.parse().ok().ok_or_not_found()?);
    Ok(view! {
        (live! {
            loop {
                let details = ingest_id::details(cx, id).await?;
                let running = ingest_id::is_running(&details);
                let token = if running {
                    emit! { ingest_progress_bar(details: &details) }?
                } else {
                    emit! { ingest_state(details: &details) }?
                };
                if !running || !connected(cx) {
                    break Ok(token);
                }
                tokio::time::sleep(ingest_id::REFRESH).await;
            }
        })
    })
}

/// A running ingest's elapsed time, counting up until the ingest stops, when
/// it becomes the time the ingest took.
///
/// Like [`live_ingest_state`], `ingest_id` is a string from the browser.
#[shard]
async fn live_ingest_elapsed(cx: &Cx, ingest_id: String) -> Result<impl View> {
    let id = IngestUuid(ingest_id.parse().ok().ok_or_not_found()?);
    Ok(view! {
        (live! {
            loop {
                let details = ingest_id::details(cx, id).await?;
                let token = emit! { (display::human_duration(details.elapsed())) }?;
                if !ingest_id::is_running(&details) || !connected(cx) {
                    break Ok(token);
                }
                tokio::time::sleep(ingest_id::REFRESH).await;
            }
        })
    })
}

/// An ingest's status, colored by outcome, and how many files it couldn't
/// measure, if any: a success that's missing files is still a success, as
/// interpretations can read it, but worth a look. Also shown on an ingest's
/// own page.
#[component]
async fn ingest_state(details: &IngestStatus) -> Result<impl View> {
    let state = details.state();
    let variant = match state {
        IngestState::Pending => BadgeVariant::Secondary,
        IngestState::Success => BadgeVariant::Success,
        IngestState::Failed => BadgeVariant::Destructive,
    };
    let failed = details.files_failed();
    Ok(view! {
        <div class="flex flex-wrap items-center gap-1">
            badge(variant: variant, (state.to_string()))
            if failed > 0 {
                badge(
                    variant: BadgeVariant::Warning,
                    attrs: attributes! {
                        title="Files that couldn't be read, so this ingest has no measurements of them"
                    },
                    (if failed == 1 {
                        "1 file failed".to_string()
                    } else {
                        format!("{failed} files failed")
                    })
                )
            }
        </div>
    })
}

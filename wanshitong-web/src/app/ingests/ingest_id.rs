use super::{ingest_progress_bar, ingest_state};
use crate::app::page_help::page_help;
use crate::{
    app::data_sources::data_source_id::data_source_summary,
    context::app,
    display,
    styles::{DETAILS, H1, H2, MONO},
    ui::{
        card::{card, card_content, card_description, card_header, card_title},
        table::{table, table_body, table_cell, table_head, table_header, table_row},
    },
};
use std::time::Duration;
use topcoat::{
    Result,
    context::Cx,
    router::{error::RouterErrorExt, page, path_param},
    runtime::{connected, shard},
    view::{EmitToken, View, attributes, component, emit, live, view},
};
use uuid::Uuid;
use wanshitong_app::{
    domain::IngestFileFailure,
    ingest::{IngestStatus, IngestUuid},
    queries,
};

path_param!(pub ingest_id: Uuid, error = not_found);

/// How often a running ingest's progress is re-read and pushed to the page.
pub(super) const REFRESH: Duration = Duration::from_secs(1);

/// An ingest's progress and event log.
///
/// The summary and event log are live regions of a shard. While the ingest
/// runs, the browser connects the shard over a WebSocket and the server pushes
/// each change to it; once the ingest stops, the regions stop too.
#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let id = IngestUuid(*path_param::<IngestId>(cx)?);
    let details = details(cx, id).await?;

    Ok(view! {
        <div class="flex items-center gap-2">
            <h1 class=(H1)>"Ingest Details"</h1>
            page_help(
                <p>
                    "A snapshot of measurements of a data source's files. "
                    "While ingest runs, its progress updates live. "
                    "A file that couldn't be read is listed under failed files and "
                    "left out, and the rest of the ingest goes on. "
                    "Its event log lists anything else that went wrong, like "
                    "a file measurement that couldn't be recorded."
                </p>
                <p>
                    "The snapshot is kept, so interpretations can read them again without "
                    "another ingest."
                </p>
            )
        </div>
        ingest_progress(ingest_id: id.0.to_string())

        <section class="flex flex-col gap-2">
            <h2 class=(H2)>"Data Source"</h2>
            <details class=(DETAILS)>
                <summary translate="no">
                    (details.data_source.name.as_str())
                </summary>
                data_source_summary(
                    uuid: details.data_source.uuid,
                    created_at: &details.data_source.created_at,
                    config: &details.data_source.config
                )
            </details>
        </section>
    })
}

/// The ingest's summary, followed by its failed files and its event log once
/// it has any. Each is a live region that re-renders while the ingest runs.
///
/// The shard has its own endpoint, so `ingest_id` comes from the browser and is
/// checked like any other request input. It is a string because shard arguments
/// must be types the browser runtime can represent, and `Uuid` isn't one.
#[shard]
async fn ingest_progress(cx: &Cx, ingest_id: String) -> Result<impl View> {
    let id = IngestUuid(ingest_id.parse().ok().ok_or_not_found()?);
    // Fails the shard early for an unknown ingest; the regions re-read it.
    details(cx, id).await?;

    Ok(view! {
        <div class="flex flex-col gap-6">
            (live! {
                loop {
                    let details = details(cx, id).await?;
                    let token = emit! { summary(details: &details) }?;
                    if !is_running(&details) || !connected(cx) {
                        break Ok(token);
                    }
                    tokio::time::sleep(REFRESH).await;
                }
            })

            (live! {
                // Failures only accrue, so they are sent again only when
                // there are more.
                let mut shown = None;
                loop {
                    let details = details(cx, id).await?;
                    let failed = details.files_failed();
                    let token = if shown != Some(failed) {
                        shown = Some(failed);
                        let failures = queries::list_ingest_file_failures(app(cx), id).await?;
                        emit! { failed_files(failures: &failures) }?
                    } else {
                        EmitToken
                    };
                    if !is_running(&details) || !connected(cx) {
                        break Ok(token);
                    }
                    tokio::time::sleep(REFRESH).await;
                }
            })

            (live! {
                // The log only grows, so it is sent again only when it has.
                let mut shown = None;
                loop {
                    let details = details(cx, id).await?;
                    let events = details
                        .progress
                        .as_ref()
                        .map_or(0, |p| p.logged_events.len());
                    let token = if shown != Some(events) {
                        shown = Some(events);
                        emit! { event_log(details: &details) }?
                    } else {
                        EmitToken
                    };
                    if !is_running(&details) || !connected(cx) {
                        break Ok(token);
                    }
                    tokio::time::sleep(REFRESH).await;
                }
            })
        </div>
    })
}

pub(super) async fn details(cx: &Cx, id: IngestUuid) -> Result<IngestStatus> {
    Ok(queries::get_ingest(app(cx), id).await?.ok_or_not_found()?)
}

/// Whether the ingest can still change. A finished one needs no connection.
pub(super) fn is_running(details: &IngestStatus) -> bool {
    details.progress.is_some() && details.stopped_at().is_none()
}

#[component]
async fn summary(details: &IngestStatus) -> Result<impl View> {
    Ok(view! {
        card(
            card_header(
                <div class="flex items-center justify-between">
                    card_title("Summary")
                    ingest_state(details: details)
                </div>
            )
            card_content(
                attrs: attributes! { class="flex flex-col gap-4" },
                // Once it stops, the status above says how it went.
                if is_running(details) {
                    ingest_progress_bar(details: details)
                }
                <dl class="kv-list">
                    <div>
                        <dt>"UUID"</dt>
                        <dd class=(MONO) translate="no">
                            (details.ingest.uuid.to_string())
                        </dd>
                    </div>

                    <div>
                        <dt>"Started"</dt>
                        <dd class=(MONO) translate="no">
                            (display::ts(&details.started_at()).to_string())
                        </dd>
                    </div>

                    <div>
                        <dt>"Stopped"</dt>
                        <dd class=(MONO) translate="no">
                            (details
                                .stopped_at()
                                .map_or(
                                    "—".to_string(),
                                    |ts| display::ts(&ts).to_string(),
                                ))
                        </dd>
                    </div>

                    <div>
                        <dt>"Elapsed"</dt>
                        <dd>(display::human_duration(details.elapsed()))</dd>
                    </div>

                    <div>
                        <dt>"Files seen"</dt>
                        <dd class="tabular-nums">(details.files_seen().to_string())</dd>
                    </div>

                    <div>
                        <dt>"Files probed"</dt>
                        <dd class="tabular-nums">
                            (details.files_probed().to_string())
                        </dd>
                    </div>

                    <div>
                        <dt>"Files recorded"</dt>
                        <dd class="tabular-nums">
                            (details.files_recorded().to_string())
                        </dd>
                    </div>

                    <div>
                        <dt>"Media files recorded"</dt>
                        <dd class="tabular-nums">
                            (details.media_files_recorded().to_string())
                        </dd>
                    </div>

                    <div>
                        <dt>"Files failed"</dt>
                        <dd class="tabular-nums">
                            (details.files_failed().to_string())
                        </dd>
                    </div>
                </dl>
            )
        )
    })
}

/// The files the ingest saw but couldn't measure, which it went on without.
#[component]
async fn failed_files(failures: &[IngestFileFailure]) -> Result<impl View> {
    Ok(view! {
        if !failures.is_empty() {
            card(
                card_header(
                    card_title("Failed Files")
                    card_description(
                        "These files couldn't be read, so this ingest has no measurements of them. "
                        "Fix what's wrong and ingest again to include them."
                    )
                )
                card_content(
                    table(
                        table_header(
                            table_row(
                                table_head("Path")
                                table_head("Stage")
                                table_head("Error")
                            )
                        )
                        table_body(
                            for failure in failures {
                                table_row(
                                    table_cell(
                                        attrs: attributes! {
                                            class=(format!("{MONO} whitespace-normal! break-all"))
                                            translate="no"
                                        },
                                        (failure.rel_path.to_string_lossy().into_owned())
                                    )
                                    table_cell(
                                        attrs: attributes! { translate="no" },
                                        (failure.stage.to_string())
                                    )
                                    table_cell(
                                        attrs: attributes! {
                                            class=(format!("{MONO} whitespace-normal! break-all"))
                                            translate="no"
                                        },
                                        (failure.error.as_str())
                                    )
                                )
                            }
                        )
                    )
                )
            )
        }
    })
}

#[component]
async fn event_log(details: &IngestStatus) -> Result<impl View> {
    Ok(view! {
        if let Some(progress) = details
            .progress
            .as_ref()
            .filter(|p| !p.logged_events.is_empty()) {
            card(
                card_header(card_title("Event Log"))
                card_content(
                    table(
                        table_header(
                            table_row(
                                table_head("Time")
                                table_head("Event")
                                table_head("Message")
                            )
                        )
                        table_body(
                            for event in &progress.logged_events {
                                table_row(
                                    table_cell(
                                        attrs: attributes! { class=(MONO) translate="no" },
                                        (display::ts(&event.time).to_string())
                                    )
                                    table_cell(
                                        attrs: attributes! { translate="no" },
                                        (event.name.to_string())
                                    )
                                    table_cell(
                                        attrs: attributes! {
                                            class=(format!("{MONO} whitespace-normal! break-all"))
                                            translate="no"
                                        },
                                        (event.payload.to_string())
                                    )
                                )
                            }
                        )
                    )
                )
            )
        }
    })
}

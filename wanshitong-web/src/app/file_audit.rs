use crate::{
    display,
    styles::{LINK, NUMBER},
    ui::table::{table, table_body, table_cell, table_head, table_header, table_row},
};
use topcoat::{
    Result,
    view::{View, attributes, component, view},
};
use wanshitong_app::domain::{
    ExternalId, KbLookupResult, KbQueryEvent, KbSearchResult, SubjectAuditLog,
};

/// The knowledge base queries and claims made about a file.
#[component]
pub async fn audit_log(audit: &SubjectAuditLog) -> Result<impl View> {
    Ok(view! {
        <div class="flex flex-col gap-4">
            if !audit.kb_queries.is_empty() {
                <div>
                    <h4 class="mb-1 font-medium">"Knowledge Base Queries"</h4>
                    <ul class="list-inside list-disc text-muted-foreground">
                        for query in &audit.kb_queries {
                            <li>
                                display::kb_query(query: query)
                                search_results(query: query)
                                lookup_result(query: query)
                            </li>
                        }
                    </ul>
                </div>
            }

            if !audit.claims.is_empty() {
                <div>
                    <h4 class="mb-1 font-medium">"Claims"</h4>
                    table(
                        attrs: attributes! { class="stacked-table" },
                        table_header(
                            table_row(
                                table_head("Active")
                                table_head("Rank")
                                table_head("Claim")
                                table_head("Source")
                            )
                        )
                        table_body(
                            for claim in &audit.claims {
                                table_row(
                                    table_cell(
                                        attrs: attributes! { data-label="Active" },
                                        (if claim.active { "Yes" } else { "No" })
                                    )
                                    table_cell(
                                        attrs: attributes! { class=(NUMBER) data-label="Rank" },
                                        (format!("{:.1}", claim.rank))
                                    )
                                    table_cell(
                                        attrs: attributes! { data-label="Claim" },
                                        display::claim_pred(pred: &claim.pred)
                                    )
                                    table_cell(
                                        attrs: attributes! { data-label="Source" },
                                        (display::claim_source(&claim.source))
                                    )
                                )
                            }
                        )
                    )
                </div>
            }

            if audit.kb_queries.is_empty() && audit.claims.is_empty() {
                <p class="text-muted-foreground">
                    "No audit data available for this file."
                </p>
            }
        </div>
    })
}

/// What a search found, in the knowledge base's order, each linked to its page.
#[component]
async fn search_results(query: &KbQueryEvent) -> Result<impl View> {
    let (results, kind): (&[KbSearchResult], _) = match query {
        KbQueryEvent::MovieSearch { results, .. } => (results, Some("movie")),
        KbQueryEvent::TvSeriesSearch { results, .. } => (results, Some("tv")),
        _ => (&[], None),
    };
    Ok(view! {
        if let Some(kind) = kind {
            if results.is_empty() {
                <p class="ml-6">"No results"</p>
            } else {
                <ol class="ml-6 list-inside list-decimal">
                    for result in results {
                        <li>
                            if let ExternalId::Tmdb(id) = result.id {
                                <a
                                    class=(LINK)
                                    translate="no"
                                    href=(format!("https://www.themoviedb.org/{kind}/{id}"))
                                >
                                    (result.title.as_str())
                                </a>
                            } else {
                                <span translate="no">(result.title.as_str())</span>
                            }
                            if let Some(year) = result.year {
                                (format!(" ({year})"))
                            }
                            " · "
                            <span translate="no">
                                (display::external_id(&result.id))
                            </span>
                        </li>
                    }
                </ol>
            }
        }
    })
}

/// What a lookup by an ID found, linked to its page, with its runtime if it's
/// a movie's, as runtime matching weighs it. Nothing for a log from before
/// lookups recorded it.
#[component]
async fn lookup_result(query: &KbQueryEvent) -> Result<impl View> {
    let (found, kind): (Option<&KbLookupResult>, _) = match query {
        KbQueryEvent::MovieDetails { found, .. } => (found.as_ref(), Some("movie")),
        KbQueryEvent::TvSeriesDetails { found, .. } => (found.as_ref(), Some("tv")),
        KbQueryEvent::Find { found, .. } => (found.as_ref(), None),
        _ => (None, None),
    };
    Ok(view! {
        if let Some(found) = found {
            <p class="ml-6">
                if let (ExternalId::Tmdb(id), Some(kind)) = (&found.id, kind) {
                    <a
                        class=(LINK)
                        translate="no"
                        href=(format!("https://www.themoviedb.org/{kind}/{id}"))
                    >
                        (found.title.as_str())
                    </a>
                } else {
                    <span translate="no">(found.title.as_str())</span>
                }
                if let Some(year) = found.year {
                    (format!(" ({year})"))
                }
                if let Some(minutes) = found.runtime_minutes {
                    (format!(" · {minutes} min"))
                }
                " · "
                <span translate="no">(display::external_id(&found.id))</span>
            </p>
        }
    })
}

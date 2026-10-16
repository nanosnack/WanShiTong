use super::{canonical_badge, interpretation_state};
use crate::app::page_help::page_help;
use crate::app::{assertions, draft};
use crate::{
    app::assertions::{RowAction, assertions_table},
    app::files::source_id::{
        SourceId,
        rel_path::{self, RelPath, segments},
    },
    app::ingests::ingest_id::{self, IngestId},
    context::app,
    display,
    icons::{CHECK, PENCIL, X, lucide},
    styles::{FACT_LABEL, FACTS, H1, H2, LINK, MONO},
    ui::{
        alert::{AlertVariant, alert, alert_description},
        button::{ButtonSize, ButtonVariant, button, button_variants},
        card::{card, card_content, card_description, card_footer, card_header, card_title},
        input::input,
        table::{table, table_body, table_cell, table_head, table_header, table_row},
    },
};
use serde::Deserialize;
use std::time::Duration;
use topcoat::{
    Result,
    context::Cx,
    router::{
        StatusCode,
        content::Form,
        error::{RouterErrorExt, see_other},
        href, page, path_param,
    },
    runtime::{Event, connected, expr, shard, signal},
    view::{View, attributes, class, component, emit, live, view},
};
use uuid::Uuid;
use wanshitong_app::{
    commands,
    domain::{
        Assertion, ChangesetState, InterpretationChangeset, InterpretationError, InterpretationUuid,
    },
    interpretations::{InterpretationState, InterpretationStatus},
    queries,
};

path_param!(pub interpretation_id: Uuid, error = not_found);

/// How often a running interpretation is re-read and pushed to the page.
const REFRESH: Duration = Duration::from_secs(1);

#[derive(Deserialize)]
struct RenameForm {
    name: String,
}

/// An interpretation's state and ingests, with a title that can be edited to
/// rename it and a form to make it canonical.
#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let id = InterpretationUuid(*path_param::<InterpretationId>(cx)?);
    let details = details(cx, id).await?;
    let assertions = queries::list_interpretation_assertions(app(cx), id).await?;
    let name = details.interpretation.name.clone();
    Ok(view! {
        interpretation_page(
            details: &details,
            assertions: &assertions,
            name: name,
            rename_error: None,
            canonical_error: None
        )
    })
}

/// Renames the interpretation. A rejected name shows the page again with the
/// name as submitted and the reason.
#[page(POST "./rename")]
async fn rename(cx: &Cx, Form(form): Form<RenameForm>) -> Result<impl View> {
    let id = InterpretationUuid(*path_param::<InterpretationId>(cx)?);
    let error = match commands::rename_interpretation(app(cx), id, &form.name).await {
        Ok(()) => return Err(see_other(href!(page, InterpretationId(id.0)).resolve(cx)).into()),
        Err(error) => error,
    };
    let details = details(cx, id).await?;
    let assertions = queries::list_interpretation_assertions(app(cx), id).await?;

    Ok(view! {
        (StatusCode::UNPROCESSABLE_ENTITY)
        interpretation_page(
            details: &details,
            assertions: &assertions,
            name: form.name,
            rename_error: Some(error.to_string()),
            canonical_error: None
        )
    })
}

/// Makes the interpretation canonical. If it can't be, the page says why.
#[page(POST "./make-canonical")]
async fn make_canonical(cx: &Cx) -> Result<impl View> {
    let id = InterpretationUuid(*path_param::<InterpretationId>(cx)?);
    let error = match commands::make_interpretation_canonical(app(cx), id).await {
        Ok(()) => return Err(see_other(href!(page, InterpretationId(id.0)).resolve(cx)).into()),
        Err(error) => error,
    };
    let details = details(cx, id).await?;
    let assertions = queries::list_interpretation_assertions(app(cx), id).await?;
    let name = details.interpretation.name.clone();

    Ok(view! {
        (StatusCode::CONFLICT)
        interpretation_page(
            details: &details,
            assertions: &assertions,
            name: name,
            rename_error: None,
            canonical_error: Some(error.to_string())
        )
    })
}

async fn details(cx: &Cx, id: InterpretationUuid) -> Result<InterpretationStatus> {
    Ok(queries::get_interpretation(app(cx), id)
        .await?
        .ok_or_not_found()?)
}

#[component]
async fn interpretation_page(
    cx: &Cx,
    details: &InterpretationStatus,
    assertions: &[Assertion],
    name: String,
    rename_error: Option<String>,
    canonical_error: Option<String>,
) -> Result<impl View> {
    let interpretation = &details.interpretation;
    let id = InterpretationId(interpretation.uuid.0);
    let current_name = interpretation.name.clone();
    // Whether the page opens with the title's rename form showing: a rejected
    // name opens it again.
    let rejected = rename_error.is_some();
    // Whether the form is showing, rather than the title.
    let editing = signal(cx, || rejected);
    // The form's field, and whether it holds the rejected name. Cancelling
    // puts the current name back.
    let field = signal(cx, || name);
    let invalid = signal(cx, || rejected);
    // Opens the form, with the field selected. The runtime applies the signal
    // in a microtask, so one queued after it runs once the form shows.
    let edit = expr!(|_e: Event| {
        editing.set(true);
        raw!("queueMicrotask(() => document.getElementById('rename-name')?.select())")
    });

    Ok(view! {
        // The title, with a button that swaps it for a form to rename the
        // interpretation.
        <div class="flex flex-col gap-2">
            <div class="flex items-center gap-2" :hidden=$(editing.get())>
                <h1 class=(H1)>
                    "Interpretation "
                    <span translate="no">(format!("\"{}\"", interpretation.name))</span>
                </h1>
                page_help(
                    <p>
                        "One run of the interpretation engine and the catalog it built. "
                        "It read the ingests and applied the assertions listed below. "
                        "Anything it couldn't do along the way, like a knowledge base "
                        "search that failed, is listed under errors: the catalog is "
                        "complete but for those."
                    </p>
                    <p>
                        "Make it canonical to browse its catalog and refresh views from it. "
                        "Rename it to tell it apart from the others."
                    </p>
                )
                button(
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Icon,
                    attrs: attributes! {
                        type="button"
                        aria-label="Rename"
                        title="Rename"
                        @click=(edit)
                    },
                    lucide(icon: PENCIL, attrs: attributes! { class="size-4" })
                )
            </div>

            <form
                class="flex items-center gap-2"
                method="post"
                action=(href!(rename, id))
                :hidden=$(!editing.get())
            >
                input(
                    attrs: attributes! {
                        id="rename-name"
                        class="max-w-md font-semibold"
                        name="name"
                        :value=$(field.get())
                        @input=$(|e: Event| field.set(e.target.value))
                        aria-label="Name"
                        :aria-invalid=$(invalid.get().then_some("true"))
                        autofocus=(rejected)
                        @keydown=$(|e: Event| {
                            if e.key == "Escape" {
                                editing.set(false);
                                field.set(current_name);
                                invalid.set(false);
                            }
                        })
                    }
                )
                button(
                    size: ButtonSize::Icon,
                    attrs: attributes! { type="submit" aria-label="Save name" title="Save name" },
                    lucide(icon: CHECK, attrs: attributes! { class="size-4" })
                )
                button(
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Icon,
                    attrs: attributes! {
                        type="button"
                        aria-label="Cancel"
                        title="Cancel"
                        @click=$(|_e: Event| {
                            editing.set(false);
                            field.set(current_name);
                            invalid.set(false);
                        })
                    },
                    lucide(icon: X, attrs: attributes! { class="size-4" })
                )
            </form>

            if let Some(error) = rename_error {
                <div :hidden=$(!editing.get())>
                    alert(
                        variant: AlertVariant::Destructive,
                        alert_description((error))
                    )
                </div>
            }
        </div>

        if let Some(error) = canonical_error {
            alert(variant: AlertVariant::Destructive, alert_description((error)))
        }

        interpretation_status(interpretation_id: interpretation.uuid.to_string())

        if let Some(changeset) = &interpretation.changeset {
            interpretation_changeset_section(changeset: changeset)
        }

        <section class="flex flex-col gap-2">
            <h2 class=(H2)>"Ingests"</h2>
            table(
                attrs: attributes! { class="stacked-table" },
                table_header(
                    table_row(
                        table_head("Data source")
                        table_head("Ingest started")
                    )
                )
                table_body(
                    for ingest in &interpretation.ingests {
                        table_row(
                            table_cell(
                                attrs: attributes! { translate="no" },
                                (ingest.data_source_name.as_str())
                            )
                            table_cell(
                                attrs: attributes! { class=(MONO) translate="no" data-label="Ingest started" },
                                <a
                                    class=(LINK)
                                    href=(href!(ingest_id::page, IngestId(ingest.ingest_uuid.0)))
                                >
                                    (display::ts(&ingest.ingest_started_at).to_string())
                                </a>
                            )
                        )
                    }
                )
            )
        </section>

        <section class="flex flex-col gap-2">
            <h2 class=(H2)>"Assertions"</h2>
            if assertions.is_empty() {
                <p class="text-muted-foreground">
                    "This interpretation includes no assertions."
                </p>
            } else {
                assertions_table(assertions: assertions, action: RowAction::None)
            }
        </section>
    })
}

/// The changeset the interpretation is of, as labelled facts, with a button
/// to review it where it still can be: an open one among the drafts, an
/// accepted one's assertions in effect.
#[component]
async fn interpretation_changeset_section(
    cx: &Cx,
    changeset: &InterpretationChangeset,
) -> Result<impl View> {
    let description = match changeset.description.trim() {
        "" => None,
        description => Some(description.to_string()),
    };
    let (state, review) = match changeset.state {
        ChangesetState::Open => (
            "Open",
            Some(("Review", draft::changeset_href(cx, changeset.uuid))),
        ),
        ChangesetState::Accepted => (
            "Accepted",
            Some(("Assertions", assertions::changeset_href(cx, changeset.uuid))),
        ),
        ChangesetState::Discarded => ("Discarded", None),
    };
    Ok(view! {
        <section class="flex flex-col gap-2">
            <div class="flex flex-wrap items-center gap-2">
                <h2 class=(class!(H2, "mr-auto"))>"Changeset"</h2>
                if let Some((label, href)) = review {
                    <a class=(button_variants(ButtonVariant::Outline, ButtonSize::Sm)) href=(href)>
                        (label)
                    </a>
                }
            </div>
            <dl class=(FACTS)>
                if let Some(description) = &description {
                    <dt class=(FACT_LABEL)>"Description"</dt>
                    <dd class="[overflow-wrap:anywhere]" translate="no">(description.as_str())</dd>
                }
                <dt class=(FACT_LABEL)>"Author"</dt>
                <dd class="[overflow-wrap:anywhere]" translate="no">(changeset.author.as_str())</dd>
                <dt class=(FACT_LABEL)>"State"</dt>
                <dd>(state)</dd>
            </dl>
        </section>
    })
}

/// The interpretation's summary and whether it's canonical, as a live region
/// that re-renders while the interpretation runs.
///
/// The shard has its own endpoint, so `interpretation_id` comes from the
/// browser and is checked like any other request input. It is a string because
/// shard arguments must be types the browser runtime can represent, and `Uuid`
/// isn't one.
#[shard]
async fn interpretation_status(cx: &Cx, interpretation_id: String) -> Result<impl View> {
    let id = InterpretationUuid(interpretation_id.parse().ok().ok_or_not_found()?);
    // Fails the shard early for an unknown interpretation; the region re-reads it.
    details(cx, id).await?;

    Ok(view! {
        <div class="flex flex-col gap-6">
            (live! {
                loop {
                    let details = details(cx, id).await?;
                    let errors = if details.interpretation.error_count > 0 {
                        queries::list_interpretation_errors(app(cx), id).await?
                    } else {
                        vec![]
                    };
                    let token = emit! {
                        summary(details: &details)
                        canonical(details: &details)
                        error_list(errors: &errors)
                    }?;
                    if details.state != InterpretationState::Running || !connected(cx) {
                        break Ok(token);
                    }
                    tokio::time::sleep(REFRESH).await;
                }
            })
        </div>
    })
}

#[component]
async fn summary(details: &InterpretationStatus) -> Result<impl View> {
    let interpretation = &details.interpretation;

    Ok(view! {
        card(
            card_header(
                <div class="flex items-center justify-between">
                    card_title("Summary")
                    interpretation_state(details: details)
                </div>
            )
            card_content(
                <div class="flex flex-col gap-4">
                    <dl class="kv-list">
                        <div>
                            <dt>"UUID"</dt>
                            <dd class=(MONO) translate="no">
                                (interpretation.uuid.to_string())
                            </dd>
                        </div>

                        <div>
                            <dt>"Started"</dt>
                            <dd class=(MONO) translate="no">
                                (display::ts(&interpretation.started_at).to_string())
                            </dd>
                        </div>

                        <div>
                            <dt>"Stopped"</dt>
                            <dd class=(MONO) translate="no">
                                (interpretation.stopped_at.map_or(
                                    "—".to_string(),
                                    |ts| display::ts(&ts).to_string(),
                                ))
                            </dd>
                        </div>

                        <div>
                            <dt>"Movies"</dt>
                            <dd class="tabular-nums">
                                (interpretation.movie_count.to_string())
                            </dd>
                        </div>

                        <div>
                            <dt>"Errors"</dt>
                            <dd class="tabular-nums">
                                (interpretation.error_count.to_string())
                            </dd>
                        </div>
                    </dl>

                    if let Some(error) = &interpretation.error {
                        alert(
                            variant: AlertVariant::Destructive,
                            alert_description((error.as_str()))
                        )
                    }
                </div>
            )
        )
    })
}

/// What the interpretation couldn't do, and went on without.
#[component]
async fn error_list(errors: &[InterpretationError]) -> Result<impl View> {
    Ok(view! {
        if !errors.is_empty() {
            card(
                card_header(
                    card_title("Errors")
                    card_description(
                        "The interpretation went on without these, so its catalog is "
                        "complete but for them. Interpret again once what's wrong is fixed."
                    )
                )
                card_content(
                    table(
                        table_header(
                            table_row(
                                table_head("While")
                                table_head("Error")
                            )
                        )
                        table_body(
                            for error in errors {
                                table_row(
                                    table_cell(
                                        attrs: attributes! {
                                            class="whitespace-normal! [overflow-wrap:anywhere]"
                                            translate="no"
                                        },
                                        if let Some(file) = &error.file {
                                            <a
                                                class=(LINK)
                                                href=(href!(
                                                    rel_path::page,
                                                    SourceId(file.data_source.0),
                                                    RelPath(segments(&file.rel_path)),
                                                ))
                                            >
                                                (error.subject.as_str())
                                            </a>
                                        } else {
                                            (error.subject.as_str())
                                        }
                                    )
                                    table_cell(
                                        attrs: attributes! {
                                            class=(format!("{MONO} whitespace-normal! break-all"))
                                            translate="no"
                                        },
                                        (error.error.as_str())
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
async fn canonical(details: &InterpretationStatus) -> Result<impl View> {
    let interpretation = &details.interpretation;
    let id = InterpretationId(interpretation.uuid.0);

    Ok(view! {
        card(
            card_header(
                <div class="flex items-center justify-between">
                    card_title("Canonical")
                    if interpretation.is_canonical {
                        canonical_badge()
                    }
                </div>
                card_description(
                    "The canonical interpretation's catalog is the one you browse, "
                    "and the one views are refreshed from."
                )
            )
            if !interpretation.is_canonical {
                card_footer(
                    if details.state == InterpretationState::Succeeded {
                        <form method="post" action=(href!(make_canonical, id))>
                            button(
                                attrs: attributes! { type="submit" },
                                "Make canonical"
                            )
                        </form>
                    } else {
                        <p class="text-sm text-muted-foreground">
                            "Only an interpretation that succeeded can be made canonical."
                        </p>
                    }
                )
            }
        )
    })
}

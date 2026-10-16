pub mod interpretation_id;

use crate::app::page_help::page_help;
use crate::{
    app::form_dialog::form_dialog,
    context::app,
    display,
    styles::{LINK, MONO, SECTION_H1},
    ui::{
        alert::{AlertVariant, alert, alert_description},
        badge::{BadgeVariant, badge},
        checkbox::checkbox,
        field::{FieldOrientation, field, field_group, field_label},
        input::input,
        table::{table, table_body, table_cell, table_head, table_header, table_row},
    },
};
use interpretation_id::InterpretationId;
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{StatusCode, content::Form, error::see_other, href, page},
    view::{View, attributes, component, view},
};
use wanshitong_app::{
    commands,
    interpretations::{InterpretationState, InterpretationStatus},
    queries,
};

#[derive(Deserialize)]
struct CreateForm {
    name: String,
    // The checkbox sends its value ("true") when checked and nothing otherwise.
    #[serde(default)]
    make_canonical: bool,
}

impl Default for CreateForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            make_canonical: true,
        }
    }
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let interpretations = queries::list_interpretations(app(cx)).await?;
    let form = CreateForm::default();
    Ok(view! {
        interpretations_page(
            interpretations: &interpretations,
            form: &form,
            error: None
        )
    })
}

/// Starts an interpretation, then shows it. A rejected one shows the list
/// again with the form as submitted and the reason.
#[page(POST)]
async fn create(cx: &Cx, Form(form): Form<CreateForm>) -> Result<impl View> {
    let error =
        match commands::create_interpretation(app(cx), &form.name, form.make_canonical).await {
            Ok(uuid) => {
                return Err(see_other(
                    href!(interpretation_id::page, InterpretationId(uuid.0)).resolve(cx),
                )
                .into());
            }
            Err(error) => error,
        };
    let interpretations = queries::list_interpretations(app(cx)).await?;

    Ok(view! {
        (StatusCode::UNPROCESSABLE_ENTITY)
        interpretations_page(
            interpretations: &interpretations,
            form: &form,
            error: Some(error.to_string())
        )
    })
}

#[component]
async fn interpretations_page(
    interpretations: &[InterpretationStatus],
    form: &CreateForm,
    error: Option<String>,
) -> Result<impl View> {
    Ok(view! {
        <div class="flex items-center gap-2">
            <h1 class=(SECTION_H1)>"Interpretations"</h1>
            page_help(
                section: true,
                <p>
                    "An interpretation identifies your files "
                    "based on measurements recorded by ingests, "
                    "with help from TMDB and your assertions, and builds a catalog "
                    "from that. It never re-reads your files, so it's quick to run another."
                </p>
                <p>
                    "The canonical interpretation's catalog is the one you browse, and the "
                    "one views are refreshed from. One starts whenever what it reads "
                    "changes: after every ingest, every accepted changeset, and a data "
                    "source's deletion or restoration. It becomes canonical when it "
                    "succeeds. One made to preview a changeset only shows what accepting "
                    "it would change."
                </p>
            )
            form_dialog(
                trigger: "New interpretation",
                title: "New interpretation",
                description: Some(
                    concat!(
                        "Builds a catalog from the latest ingest of every data source. ",
                        "Name it after what sets it apart, ",
                        "so you can tell interpretations apart later.",
                    ),
                ),
                form_id: "new-interpretation",
                submit: "Start",
                open: error.is_some(),
                attrs: attributes! { class="ml-auto" },
                <form id="new-interpretation" method="post" action=(href!(page))>
                    field_group(
                        if let Some(error) = error {
                            alert(
                                variant: AlertVariant::Destructive,
                                alert_description((error))
                            )
                        }

                        field(
                            field_label(attrs: attributes! { for="name" }, "Name")
                            input(
                                attrs: attributes! { id="name" name="name" value=(form.name.as_str()) }
                            )
                        )

                        field(
                            orientation: FieldOrientation::Horizontal,
                            checkbox(
                                attrs: attributes! {
                                    id="make_canonical"
                                    name="make_canonical"
                                    value="true"
                                    checked=(form.make_canonical)
                                }
                            )
                            field_label(
                                attrs: attributes! { for="make_canonical" },
                                "Make canonical when finished"
                            )
                        )
                    )
                </form>
            )
        </div>

        if !interpretations.is_empty() {
            table(
                attrs: attributes! { class="stacked-table" },
                table_header(
                    table_row(
                        table_head("Name")
                        table_head("State")
                        table_head("Started")
                        table_head("Movies")
                        table_head("Episodes")
                    )
                )
                table_body(
                    for details in interpretations {
                        table_row(
                            table_cell(
                                attrs: attributes! { class="whitespace-normal!" },
                                <a
                                    class=(LINK)
                                    data-row-link=""
                                    href=(href!(
                                        interpretation_id::page,
                                        InterpretationId(details.interpretation.uuid.0),
                                    ))
                                    translate="no"
                                >
                                    (details.interpretation.name.as_str())
                                </a>
                            )
                            table_cell(
                                attrs: attributes! { data-label="State" },
                                <div class="flex flex-wrap gap-1">
                                    interpretation_state(details: details)
                                    if details.interpretation.is_canonical {
                                        canonical_badge()
                                    }
                                </div>
                            )
                            table_cell(
                                attrs: attributes! { class=(MONO) translate="no" data-label="Started" },
                                (display::ts(&details.interpretation.started_at).to_string())
                            )
                            table_cell(
                                attrs: attributes! { class="tabular-nums" data-label="Movies" },
                                (details.interpretation.movie_count.to_string())
                            )
                            table_cell(
                                attrs: attributes! { class="tabular-nums" data-label="Episodes" },
                                (details.interpretation.episode_count.to_string())
                            )
                        )
                    }
                )
            )
        } else {
            <p class="text-muted-foreground">
                "You don't have any interpretations yet."
            </p>
        }
    })
}

/// Whether an interpretation's catalog is ready, and how many things it
/// couldn't do along the way, if any: its catalog is complete but for those.
/// Also shown on its own page.
#[component]
pub async fn interpretation_state(details: &InterpretationStatus) -> Result<impl View> {
    let (variant, label) = match details.state {
        InterpretationState::Running => (BadgeVariant::Secondary, "Running"),
        InterpretationState::Interrupted => (BadgeVariant::Destructive, "Interrupted"),
        InterpretationState::Failed => (BadgeVariant::Destructive, "Failed"),
        InterpretationState::Succeeded => (BadgeVariant::Success, "Succeeded"),
    };
    let errors = details.interpretation.error_count;
    Ok(view! {
        <div class="flex flex-wrap items-center gap-1">
            badge(variant: variant, (label))
            if errors > 0 {
                badge(
                    variant: BadgeVariant::Warning,
                    attrs: attributes! {
                        title="Things the interpretation couldn't do, like knowledge base searches that failed"
                    },
                    (if errors == 1 {
                        "1 error".to_string()
                    } else {
                        format!("{errors} errors")
                    })
                )
            }
        </div>
    })
}

#[component]
pub async fn canonical_badge() -> Result<impl View> {
    Ok(view! { badge(variant: BadgeVariant::Outline, "Canonical") })
}

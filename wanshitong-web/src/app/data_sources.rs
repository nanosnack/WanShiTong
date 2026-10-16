pub mod data_source_id;

use crate::app::page_help::page_help;
use crate::{
    app::{
        form_dialog::form_dialog,
        ingests::{self, ingest_id},
        path_completions::path_input,
    },
    context::app,
    display,
    styles::{DETAILS, LINK, MONO, SECTION_H1},
    ui::{
        alert::{AlertVariant, alert, alert_description},
        button::{ButtonSize, ButtonVariant, button, button_variants},
        checkbox::checkbox,
        field::{FieldOrientation, field, field_group, field_label},
        input::input,
        table::{table, table_body, table_cell, table_head, table_header, table_row},
    },
};
use serde::Deserialize;
use std::collections::HashSet;
use topcoat::{
    Result,
    context::Cx,
    router::{StatusCode, content::Form, error::see_other, href, page},
    view::{View, attributes, component, view},
};
use wanshitong_app::{
    commands,
    domain::{DataSource, DataSourceConfig, FileSystemSource},
    ingest::{IngestState, IngestUuid},
    queries,
};

#[derive(Default, Deserialize)]
struct CreateForm {
    name: String,
    root_path: String,
    // The checkboxes send their value ("true") when checked and nothing
    // otherwise.
    #[serde(default)]
    follow_symlinks: bool,
    #[serde(default)]
    start_ingest: bool,
}

impl CreateForm {
    fn blank() -> Self {
        Self {
            // A new data source is usually ingested right away.
            start_ingest: true,
            ..Default::default()
        }
    }
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let sources = queries::list_data_sources(app(cx)).await?;
    let deleted = queries::list_deleted_data_sources(app(cx)).await?;
    let running = running_ingests(cx).await?;
    let form = CreateForm::blank();
    Ok(view! {
        data_sources(
            sources: &sources,
            deleted: &deleted,
            running: &running,
            watching: app(cx).config().watch_data_sources,
            form: &form,
            error: None
        )
    })
}

/// Creates a data source and, if asked, starts ingesting it, then shows the
/// list with it. A rejected source shows the list again with the form as
/// submitted and the reason.
#[page(POST)]
async fn create(cx: &Cx, Form(form): Form<CreateForm>) -> Result<impl View> {
    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: form.root_path.clone().into(),
        follow_symlinks: form.follow_symlinks,
    });

    let (status, form, error) =
        match commands::create_data_source(app(cx), &form.name, config).await {
            Ok(source) if form.start_ingest => {
                match commands::start_ingest(app(cx), source.uuid).await {
                    Ok(_) => return Err(see_other(href!(page).resolve(cx)).into()),
                    // The source exists, so the form starts over.
                    Err(error) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        CreateForm::blank(),
                        format!(
                            "Added {}, but couldn't start ingesting it: {error}",
                            source.name
                        ),
                    ),
                }
            }
            Ok(_) => return Err(see_other(href!(page).resolve(cx)).into()),
            Err(error) => (StatusCode::UNPROCESSABLE_ENTITY, form, error.to_string()),
        };
    let sources = queries::list_data_sources(app(cx)).await?;
    let deleted = queries::list_deleted_data_sources(app(cx)).await?;
    let running = running_ingests(cx).await?;

    Ok(view! {
        (status)
        data_sources(
            sources: &sources,
            deleted: &deleted,
            running: &running,
            watching: app(cx).config().watch_data_sources,
            form: &form,
            error: Some(error)
        )
    })
}

/// Ingests that haven't finished.
async fn running_ingests(cx: &Cx) -> Result<HashSet<IngestUuid>> {
    Ok(queries::list_ingests(app(cx))
        .await?
        .into_iter()
        .filter(|ingest| matches!(ingest.state(), IngestState::Pending))
        .map(|ingest| ingest.ingest.uuid)
        .collect())
}

#[component]
async fn data_sources(
    sources: &[DataSource],
    deleted: &[DataSource],
    running: &HashSet<IngestUuid>,
    watching: bool,
    form: &CreateForm,
    error: Option<String>,
) -> Result<impl View> {
    Ok(view! {
        <div class="flex items-center gap-2">
            <h1 class=(SECTION_H1)>"Data Sources"</h1>
            page_help(
                section: true,
                <p>
                    "A data source is a directory containing your media. "
                    "Wan Shi Tong only ever reads it: "
                    "your files are never moved, renamed or changed."
                </p>
                <p>
                    if watching {
                        "Ingest a data source to read its files. After that, it's ingested "
                        "again whenever its files change."
                    } else {
                        "Ingest a data source to read its files. Ingest it again after its "
                        "files change, so the catalog catches up."
                    }
                </p>
            )
            form_dialog(
                trigger: "New data source",
                title: "Add data source",
                form_id: "add-data-source",
                submit: "Add",
                open: error.is_some(),
                attrs: attributes! { class="ml-auto" },
                <form id="add-data-source" method="post" action=(href!(page))>
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
                            field_label(
                                attrs: attributes! { for="root_path" },
                                "Root path"
                            )
                            path_input(
                                id: "root_path",
                                name: "root_path",
                                value: &form.root_path
                            )
                        )

                        field(
                            orientation: FieldOrientation::Horizontal,
                            checkbox(
                                attrs: attributes! {
                                    id="follow_symlinks"
                                    name="follow_symlinks"
                                    value="true"
                                    checked=(form.follow_symlinks)
                                }
                            )
                            field_label(
                                attrs: attributes! { for="follow_symlinks" },
                                "Follow symlinks"
                            )
                        )

                        field(
                            orientation: FieldOrientation::Horizontal,
                            checkbox(
                                attrs: attributes! {
                                    id="start_ingest"
                                    name="start_ingest"
                                    value="true"
                                    checked=(form.start_ingest)
                                }
                            )
                            field_label(
                                attrs: attributes! { for="start_ingest" },
                                "Start ingesting it now"
                            )
                        )
                    )
                </form>
            )
        </div>

        if !sources.is_empty() {
            table(
                attrs: attributes! { id="data-source-table" class="stacked-table" },
                table_header(
                    table_row(
                        table_head("Name")
                        table_head("Kind")
                        table_head("Created")
                        table_head("Details")
                        table_head()
                    )
                )
                table_body(
                    for source in sources {
                        table_row(
                            table_cell(
                                <a
                                    class=(LINK)
                                    data-row-link=""
                                    href=(href!(
                                        data_source_id::page,
                                        data_source_id::DataSourceId(source.uuid.0),
                                    ))
                                    translate="no"
                                >
                                    (source.name.as_str())
                                </a>
                            )
                            table_cell(
                                attrs: attributes! { data-label="Kind" },
                                (source.config.kind().to_string())
                            )
                            table_cell(
                                attrs: attributes! { class=(MONO) translate="no" data-label="Created" },
                                (display::ts(&source.created_at).to_string())
                            )
                            table_cell(
                                attrs: attributes! { class=(MONO) data-label="Details" },
                                display::source(config: &source.config)
                            )
                            table_cell(
                                attrs: attributes! { class="text-right max-sm:pt-2" },
                                // One ingest at a time: a running one is shown
                                // instead of starting another.
                                if let Some(ingest) = source.latest_ingest.filter(
                                    |i| running.contains(i),
                                ) {
                                    <a
                                        class=(button_variants(
                                            ButtonVariant::Outline,
                                            ButtonSize::Sm,
                                        ))
                                        href=(href!(ingest_id::page, ingest_id::IngestId(ingest.0)))
                                    >
                                        "View ingest"
                                    </a>
                                } else {
                                    // Starting an ingest swaps in the table as
                                    // it is after, rather than leaving the page.
                                    <form
                                        method="post"
                                        action=(href!(ingests::page))
                                        x-data=""
                                        x-target="data-source-table"
                                    >
                                        <input
                                            type="hidden"
                                            name="data_source_id"
                                            value=(source.uuid.to_string())
                                        >
                                        button(
                                            variant: ButtonVariant::Outline,
                                            size: ButtonSize::Sm,
                                            attrs: attributes! { type="submit" },
                                            "Ingest"
                                        )
                                    </form>
                                }
                            )
                        )
                    }
                )
            )
        } else {
            <p class="text-muted-foreground">"You don't have any data sources yet."</p>
        }

        // Out of the way, but there to restore from.
        if !deleted.is_empty() {
            <details class=(DETAILS)>
                <summary>(format!("Deleted data sources ({})", deleted.len()))</summary>
                table(
                    attrs: attributes! { class="stacked-table" },
                    table_header(
                        table_row(
                            table_head("Name")
                            table_head("Deleted")
                            table_head("Details")
                        )
                    )
                    table_body(
                        for source in deleted {
                            table_row(
                                table_cell(
                                    <a
                                        class=(LINK)
                                        data-row-link=""
                                        href=(href!(
                                            data_source_id::page,
                                            data_source_id::DataSourceId(source.uuid.0),
                                        ))
                                        translate="no"
                                    >
                                        (source.name.as_str())
                                    </a>
                                )
                                table_cell(
                                    attrs: attributes! { class=(MONO) translate="no" data-label="Deleted" },
                                    (source
                                        .deleted_at
                                        .as_ref()
                                        .map(|t| display::ts(t).to_string())
                                        .unwrap_or_default())
                                )
                                table_cell(
                                    attrs: attributes! { class=(MONO) data-label="Details" },
                                    display::source(config: &source.config)
                                )
                            )
                        }
                    )
                )
            </details>
        }
    })
}

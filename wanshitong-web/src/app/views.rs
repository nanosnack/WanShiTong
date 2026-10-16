pub mod view_id;

use super::{form_dialog::form_dialog, path_completions::path_input};
use crate::app::page_help::page_help;
use crate::{
    context::app,
    display,
    styles::{LINK, MONO, SECTION_H1},
    ui::{
        alert::{AlertVariant, alert, alert_description},
        checkbox::checkbox,
        field::{FieldOrientation, field, field_group, field_label},
        input::input,
        table::{table, table_body, table_cell, table_head, table_header, table_row},
    },
};
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{StatusCode, content::Form, error::see_other, href, page},
    view::{View, attributes, class, component, view},
};
use view_id::view_state;
use wanshitong_app::{
    commands,
    domain::{JellyfinView, PlexView, ViewConfig},
    queries,
    views::ViewStatus,
};

#[derive(Default, Deserialize)]
struct CreateForm {
    name: String,
    #[serde(default)]
    server: MediaServer,
    root_path: String,
    // The checkbox sends its value ("true") when checked and nothing
    // otherwise.
    #[serde(default)]
    auto_refresh: bool,
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum MediaServer {
    #[default]
    Jellyfin,
    Plex,
}

impl CreateForm {
    fn blank() -> Self {
        Self {
            // A view usually keeps up with the catalog.
            auto_refresh: true,
            ..Default::default()
        }
    }
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let views = queries::list_views(app(cx)).await?;
    let form = CreateForm::blank();
    Ok(view! { views_page(views: &views, form: &form, error: None) })
}

/// Creates a view, then shows it. A rejected view shows the list again with
/// the form as submitted and the reason.
#[page(POST)]
async fn create(cx: &Cx, Form(form): Form<CreateForm>) -> Result<impl View> {
    let root_path = form.root_path.trim().into();
    let config = match form.server {
        MediaServer::Jellyfin => ViewConfig::Jellyfin(JellyfinView { root_path }),
        MediaServer::Plex => ViewConfig::Plex(PlexView { root_path }),
    };

    let error = match commands::create_view(app(cx), &form.name, config, form.auto_refresh).await {
        Ok(view) => {
            return Err(
                see_other(href!(view_id::page, view_id::ViewId(view.uuid.0)).resolve(cx)).into(),
            );
        }
        Err(error) => error,
    };
    let views = queries::list_views(app(cx)).await?;

    Ok(view! {
        (StatusCode::UNPROCESSABLE_ENTITY)
        views_page(views: &views, form: &form, error: Some(error.to_string()))
    })
}

#[component]
async fn views_page(
    views: &[ViewStatus],
    form: &CreateForm,
    error: Option<String>,
) -> Result<impl View> {
    Ok(view! {
        <div class="flex items-center gap-2">
            <h1 class=(SECTION_H1)>"Views"</h1>
            page_help(
                section: true,
                <p>
                    "A view is a directory of symlinks to your media files, "
                    "laid out and named to match the conventions of a specific media server. "
                    "Add the view's directory to your media server's library "
                    "to get access to all identified works."
                </p>
                <p>
                    "A view is filled from the canonical catalog. When another "
                    "interpretation becomes canonical, the view is refreshed, unless you "
                    "turned that off; then it's stale until you refresh it."
                </p>
            )
            form_dialog(
                trigger: "New view",
                title: "Add view",
                description: Some(
                    concat!(
                        "A directory of symlinks to your media files, ",
                        "named so your media server can identify them. ",
                        "It's filled from the current catalog as soon as it's added. ",
                        "Your media files are never touched.",
                    ),
                ),
                form_id: "add-view",
                submit: "Add",
                open: error.is_some(),
                attrs: attributes! { class="ml-auto" },
                <form id="add-view" method="post" action=(href!(page))>
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
                                attrs: attributes! { for="server" },
                                "Media server"
                            )
                            <select
                                id="server"
                                name="server"
                                class=(class!(
                                    "h-9 w-full rounded-lg border border-border bg-background",
                                    "px-3 text-sm",
                                ))
                            >
                                <option
                                    value="jellyfin"
                                    selected=(form.server == MediaServer::Jellyfin)
                                >
                                    "Jellyfin"
                                </option>
                                <option
                                    value="plex"
                                    selected=(form.server == MediaServer::Plex)
                                >
                                    "Plex"
                                </option>
                            </select>
                        )

                        field(
                            field_label(
                                attrs: attributes! { for="root_path" },
                                "Root path"
                            )
                            path_input(
                                id: "root_path",
                                name: "root_path",
                                value: &form.root_path,
                                placeholder: Some("/srv/views")
                            )
                        )
                        field(
                            orientation: FieldOrientation::Horizontal,
                            checkbox(
                                attrs: attributes! {
                                    id="auto_refresh"
                                    name="auto_refresh"
                                    value="true"
                                    checked=(form.auto_refresh)
                                }
                            )
                            field_label(
                                attrs: attributes! { for="auto_refresh" },
                                "Refresh when the canonical interpretation changes"
                            )
                        )
                    )
                </form>
            )
        </div>

        if !views.is_empty() {
            table(
                attrs: attributes! { class="stacked-table" },
                table_header(
                    table_row(
                        table_head("Name")
                        table_head("Kind")
                        table_head("Root path")
                        table_head("State")
                        table_head("Last refreshed")
                    )
                )
                table_body(
                    for status in views {
                        table_row(
                            table_cell(
                                <a
                                    class=(LINK)
                                    data-row-link=""
                                    href=(href!(
                                        view_id::page,
                                        view_id::ViewId(status.view.uuid.0),
                                    ))
                                    translate="no"
                                >
                                    (status.view.name.as_str())
                                </a>
                            )
                            table_cell(
                                attrs: attributes! { data-label="Kind" },
                                (status.view.config.kind().to_string())
                            )
                            table_cell(
                                attrs: attributes! { class=(MONO) translate="no" data-label="Root path" },
                                (status.view.config.root_path().display().to_string())
                            )
                            table_cell(
                                attrs: attributes! { data-label="State" },
                                view_state(state: status.state)
                            )
                            table_cell(
                                attrs: attributes! { class=(MONO) data-label="Last refreshed" },
                                match &status.view.latest_materialization {
                                    Some(latest) => <span translate="no">
                                        (display::ts(&latest.stopped_at).to_string())
                                    </span>,
                                    None => "Never",
                                }
                            )
                        )
                    }
                )
            )
        } else {
            <p class="text-muted-foreground">"You don't have any views yet."</p>
        }
    })
}

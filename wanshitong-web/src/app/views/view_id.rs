use crate::app::page_help::page_help;
use crate::{
    context::app,
    display,
    error::PageError,
    styles::{H1, MONO},
    ui::{
        alert::{AlertVariant, alert, alert_description},
        badge::{BadgeVariant, badge},
        button::{ButtonVariant, button},
        card::{card, card_content, card_description, card_footer, card_header, card_title},
        checkbox::checkbox,
        field::{FieldOrientation, field, field_label},
    },
};
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{
        StatusCode,
        content::Form,
        error::{RouterErrorExt, SeeOther, see_other},
        href, page, path_param, route,
    },
    view::{View, attributes, component, view},
};
use uuid::Uuid;
use wanshitong_app::{
    commands,
    domain::ViewUuid,
    queries,
    views::{ViewState, ViewStatus},
};

path_param!(pub view_id: Uuid, error = not_found);

/// A view's settings and state, with buttons to bring it up to date or delete
/// it.
#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let id = ViewUuid(*path_param::<ViewId>(cx)?);
    let status = queries::get_view(app(cx), id).await?.ok_or_not_found()?;
    Ok(view! { view_details(status: status, delete_error: None) })
}

/// Refreshes the view, then shows how it went.
#[route(POST "./refresh")]
async fn refresh(cx: &Cx) -> Result<SeeOther> {
    let id = ViewUuid(*path_param::<ViewId>(cx)?);
    commands::refresh_view(app(cx), id)
        .await
        .map_err(|e| PageError::log(e.into()))?;
    Ok(see_other(href!(page, ViewId(id.0)).resolve(cx)))
}

#[derive(Deserialize)]
struct AutoRefreshForm {
    // The checkbox sends its value ("true") when checked and nothing
    // otherwise.
    #[serde(default)]
    auto_refresh: bool,
}

/// Sets whether the view is refreshed when the canonical interpretation
/// changes, then shows the view.
#[route(POST "./auto-refresh")]
async fn set_auto_refresh(cx: &Cx, Form(form): Form<AutoRefreshForm>) -> Result<SeeOther> {
    let id = ViewUuid(*path_param::<ViewId>(cx)?);
    commands::set_view_auto_refresh(app(cx), id, form.auto_refresh)
        .await
        .map_err(|e| PageError::log(e.into()))?;
    Ok(see_other(href!(page, ViewId(id.0)).resolve(cx)))
}

/// Deletes the view and its symlinks, then shows the remaining views. If the
/// symlinks can't be removed, the view is kept and its page says why.
#[page(POST "./delete")]
async fn delete(cx: &Cx) -> Result<impl View> {
    let id = ViewUuid(*path_param::<ViewId>(cx)?);
    let error = match commands::delete_view(app(cx), id).await {
        Ok(()) => return Err(see_other(href!(super::page).resolve(cx)).into()),
        Err(error) => error,
    };
    let status = queries::get_view(app(cx), id).await?.ok_or_not_found()?;

    Ok(view! {
        (StatusCode::CONFLICT)
        view_details(status: status, delete_error: Some(error.to_string()))
    })
}

#[component]
async fn view_details(status: ViewStatus, delete_error: Option<String>) -> Result<impl View> {
    let state = status.state;
    let view = status.view;

    Ok(view! {
        <div class="flex items-center gap-2">
            <h1 class=(H1)>
                "View "
                <span translate="no">(format!("\"{}\"", view.name))</span>
            </h1>
            page_help(
                <p>
                    "A directory of symlinks to your media, for a media server to read. "
                    "Point your media server's library at the "
                    "movies library and shows library paths below."
                </p>
                <p>
                    "If automatic refresh is disabled, the view becomes stale as files change. "
                    "Deleting the view removes its symlinks, never your media."
                </p>
            )
        </div>

        card(
            card_header(card_title("Summary"))
            card_content(
                <dl class="kv-list">
                    <div>
                        <dt>"UUID"</dt>
                        <dd class=(MONO) translate="no">(view.uuid.to_string())</dd>
                    </div>

                    <div>
                        <dt>"Kind"</dt>
                        <dd>(view.config.kind().to_string())</dd>
                    </div>

                    <div>
                        <dt>"Created"</dt>
                        <dd class=(MONO) translate="no">
                            (display::ts(&view.created_at).to_string())
                        </dd>
                    </div>

                    <div>
                        <dt>"Root path"</dt>
                        <dd class=(MONO) translate="no">
                            (view.config.root_path().display().to_string())
                        </dd>
                    </div>

                    <div>
                        <dt>"Movies library"</dt>
                        <dd class=(MONO) translate="no">
                            (view.config.movies_path().display().to_string())
                        </dd>
                    </div>

                    <div>
                        <dt>"Shows library"</dt>
                        <dd class=(MONO) translate="no">
                            (view.config.shows_path().display().to_string())
                        </dd>
                    </div>
                </dl>
            )
        )

        card(
            card_header(
                card_title("State")
                card_description(
                    "Refreshing makes the view match the current catalog. "
                    "Only the view's symlinks are changed."
                )
            )
            card_content(
                <div class="flex flex-col gap-4">
                    <dl class="kv-list">
                        <div>
                            <dt>"State"</dt>
                            <dd>view_state(state: state)</dd>
                        </div>

                        if let Some(latest) = &view.latest_materialization {
                            <div>
                                <dt>"Last refreshed"</dt>
                                <dd class=(MONO) translate="no">
                                    (display::ts(&latest.stopped_at).to_string())
                                </dd>
                            </div>

                            if let Ok(stats) = &latest.result {
                                <div>
                                    <dt>"Links created"</dt>
                                    <dd class=(MONO) translate="no">
                                        (stats.links_created.to_string())
                                    </dd>
                                </div>

                                <div>
                                    <dt>"Links updated"</dt>
                                    <dd class=(MONO) translate="no">
                                        (stats.links_updated.to_string())
                                    </dd>
                                </div>

                                <div>
                                    <dt>"Links removed"</dt>
                                    <dd class=(MONO) translate="no">
                                        (stats.links_removed.to_string())
                                    </dd>
                                </div>
                            }
                        }
                    </dl>

                    if let Some(Err(error)) = view
                        .latest_materialization
                        .as_ref()
                        .map(|latest| &latest.result) {
                        alert(
                            variant: AlertVariant::Destructive,
                            alert_description((error.as_str()))
                        )
                    }

                    <form
                        method="post"
                        action=(href!(set_auto_refresh, ViewId(view.uuid.0)))
                        x-data=""
                        x-on:change="$el.requestSubmit()"
                    >
                        field(
                            orientation: FieldOrientation::Horizontal,
                            checkbox(
                                attrs: attributes! {
                                    id="auto_refresh"
                                    name="auto_refresh"
                                    value="true"
                                    checked=(view.auto_refresh)
                                }
                            )
                            field_label(
                                attrs: attributes! { for="auto_refresh" },
                                "Refresh when the canonical interpretation changes"
                            )
                        )
                    </form>
                </div>
            )
            card_footer(
                <form method="post" action=(href!(refresh, ViewId(view.uuid.0)))>
                    button(attrs: attributes! { type="submit" }, "Refresh")
                </form>
            )
        )

        card(
            card_header(
                card_title("Delete")
                card_description(
                    "Removes the view and its symlinks. Your media files are never touched, "
                    "and WST stops the deletion if it finds unexpected files in the view folder."
                )
            )
            if let Some(error) = delete_error {
                card_content(
                    alert(
                        variant: AlertVariant::Destructive,
                        alert_description((error))
                    )
                )
            }
            card_footer(
                <form
                    method="post"
                    action=(href!(delete, ViewId(view.uuid.0)))
                    data-confirm="Delete this view and its symlinks?"
                    onsubmit="return confirm(this.dataset.confirm)"
                >
                    button(
                        variant: ButtonVariant::Destructive,
                        attrs: attributes! { type="submit" },
                        "Delete"
                    )
                </form>
            )
        )
    })
}

/// How a view compares to the catalog. Also shown in the list of views.
#[component]
pub async fn view_state(state: ViewState) -> Result<impl View> {
    let (variant, label) = match state {
        ViewState::NeverMaterialized => (BadgeVariant::Outline, "Never refreshed"),
        ViewState::Failed => (BadgeVariant::Destructive, "Failed"),
        ViewState::Stale => (BadgeVariant::Secondary, "Stale"),
        ViewState::UpToDate => (BadgeVariant::Success, "Up to date"),
    };
    Ok(view! { badge(variant: variant, (label)) })
}

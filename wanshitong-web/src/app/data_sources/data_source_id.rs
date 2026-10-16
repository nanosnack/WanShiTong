use crate::app::page_help::page_help;
use crate::{
    context::app,
    display,
    styles::{H1, MONO},
    ui::{
        alert::{AlertVariant, alert, alert_description},
        button::{ButtonVariant, button},
        card::{card, card_content, card_description, card_footer, card_header, card_title},
    },
};
use jiff::Timestamp;
use topcoat::{
    Result,
    context::Cx,
    router::{
        StatusCode,
        error::{RouterErrorExt, see_other},
        href, page, path_param,
    },
    view::{View, attributes, component, view},
};
use uuid::Uuid;
use wanshitong_app::{
    commands,
    domain::{DataSource, DataSourceConfig, DataSourceUuid},
    queries,
};

path_param!(pub data_source_id: Uuid, error = not_found);

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let id = DataSourceUuid(*path_param::<DataSourceId>(cx)?);
    let source = queries::get_data_source(app(cx), id)
        .await?
        .ok_or_not_found()?;
    Ok(view! { data_source_details(source: source, error: None) })
}

/// Deletes the data source, then shows the remaining ones.
#[page(POST "./delete")]
async fn delete(cx: &Cx) -> Result<impl View> {
    let id = DataSourceUuid(*path_param::<DataSourceId>(cx)?);
    let error = match commands::delete_data_source(app(cx), id).await {
        Ok(_) => return Err(see_other(href!(super::page).resolve(cx)).into()),
        Err(error) => error,
    };
    let source = queries::get_data_source(app(cx), id)
        .await?
        .ok_or_not_found()?;
    Ok(view! {
        (StatusCode::CONFLICT)
        data_source_details(source: source, error: Some(error.to_string()))
    })
}

/// Restores the deleted data source, then shows it. If its name or place has
/// been taken since, it stays deleted and its page says why.
#[page(POST "./restore")]
async fn restore(cx: &Cx) -> Result<impl View> {
    let id = DataSourceUuid(*path_param::<DataSourceId>(cx)?);
    let error = match commands::restore_data_source(app(cx), id).await {
        Ok(_) => return Err(see_other(href!(page, DataSourceId(id.0)).resolve(cx)).into()),
        Err(error) => error,
    };
    let source = queries::get_data_source(app(cx), id)
        .await?
        .ok_or_not_found()?;
    Ok(view! {
        (StatusCode::CONFLICT)
        data_source_details(source: source, error: Some(error.to_string()))
    })
}

/// A data source's settings, with a button to delete it, or to restore it if
/// it's deleted.
#[component]
async fn data_source_details(source: DataSource, error: Option<String>) -> Result<impl View> {
    Ok(view! {
        <div class="flex items-center gap-2">
            <h1 class=(H1)>
                "Data Source "
                <span translate="no">(format!("\"{}\"", source.name))</span>
            </h1>
            page_help(
                <p>"A directory containing your media."</p>
                <p>
                    "With “Follow symlinks”, symbolic links inside it are followed, so "
                    "files they point to elsewhere are read too."
                </p>
                <p>
                    "Deleting it leaves it out of new ingests and catalogs, and never "
                    "touches your media. It can be restored."
                </p>
            )
        </div>

        card(
            card_header(card_title("Summary"))
            card_content(
                data_source_summary(
                    uuid: source.uuid,
                    created_at: &source.created_at,
                    config: &source.config
                )
            )
        )

        if let Some(deleted_at) = &source.deleted_at {
            card(
                card_header(
                    card_title("Deleted")
                    card_description(
                        (format!("Deleted {}. ", display::ts(deleted_at)))
                        "It takes no part in new ingests or catalogs. Restoring it brings "
                        "back its files and assertions in a new catalog."
                    )
                )
                if let Some(error) = error {
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
                        action=(href!(restore, DataSourceId(source.uuid.0)))
                    >
                        button(attrs: attributes! { type="submit" }, "Restore")
                    </form>
                )
            )
        } else {
            card(
                card_header(
                    card_title("Delete")
                    card_description(
                        "Leaves the data source out of new ingests and catalogs; a new "
                        "catalog without it is made right away. Your media files are "
                        "never touched, and it can be restored."
                    )
                )
                if let Some(error) = error {
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
                        action=(href!(delete, DataSourceId(source.uuid.0)))
                        data-confirm="Delete this data source?"
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
        }
    })
}

/// A data source's identity and settings, as shown on its own page and in the
/// ingests that read it.
#[component]
pub async fn data_source_summary(
    uuid: DataSourceUuid,
    created_at: &Timestamp,
    config: &DataSourceConfig,
) -> Result<impl View> {
    Ok(view! {
        <div class="flex flex-col gap-4">
            <dl class="kv-list">
                <div>
                    <dt>"UUID"</dt>
                    <dd class=(MONO) translate="no">(uuid.to_string())</dd>
                </div>

                <div>
                    <dt>"Kind"</dt>
                    <dd>(config.kind().to_string())</dd>
                </div>

                <div>
                    <dt>"Created"</dt>
                    <dd class=(MONO) translate="no">
                        (display::ts(created_at).to_string())
                    </dd>
                </div>
            </dl>

            match config {
                DataSourceConfig::FileSystem(fs) => {
                    <h3 class="font-medium">"Filesystem Settings"</h3>
                    <dl class="kv-list">
                        <div>
                            <dt>"Root path"</dt>
                            <dd class=(MONO) translate="no">
                                (fs.root_path.display().to_string())
                            </dd>
                        </div>

                        <div>
                            <dt>"Follow symlinks"</dt>
                            <dd>(if fs.follow_symlinks { "Yes" } else { "No" })</dd>
                        </div>
                    </dl>
                }
            }
        </div>
    })
}

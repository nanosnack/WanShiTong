//! The accepted assertions in effect, grouped by the changeset that made them,
//! any selection of which is retractable into the draft. The third page of the
//! Corrections section (see `app::corrections`).
//! Also their backup: exporting them as a file, and importing such a file,
//! which `wst assertions export` and `import` do the same way.

use crate::api::assertions::{active_assertions, import_exported};
use crate::app::corrections::marking::SELECTED_BAR;
use crate::app::corrections::{SectionPage, section_nav};
use crate::app::draft;
use crate::app::form_dialog::form_dialog;
use crate::app::interpretations::interpretation_id::{self, InterpretationId};
use crate::app::page_help::page_help;
use crate::app::path_tree::{ListedFile, path_list};
use crate::{
    context::app,
    display,
    error::PageError,
    styles::{H2, SECTION_H1, SECTION_HEADING_ROW},
    ui::{
        alert::{AlertVariant, alert, alert_description},
        button::{ButtonSize, ButtonVariant, button, button_variants},
        checkbox::checkbox,
        field::{field, field_group, field_label},
        input::input,
        table::{table, table_body, table_cell, table_head, table_header, table_row},
    },
};
use jiff::Timestamp;
use std::collections::HashMap;
use topcoat::{
    Result,
    context::Cx,
    router::{
        Body, StatusCode,
        content::{RawForm, multipart::Multipart},
        error::{SeeOther, see_other},
        href, page,
        response::{IntoResponse, Response},
        route,
    },
    view::{View, attributes, component, view},
};
use wanshitong_api::v1;
use wanshitong_app::{
    commands::{self, ImportedAssertions, USER},
    domain::{Assertion, AssertionUuid, ChangesetDetails, ChangesetState, ChangesetUuid},
    queries,
};

/// Who an import's changeset is by.
const IMPORT_AUTHOR: &str = "import";

/// The form field the import's file comes in.
const IMPORT_FILE_FIELD: &str = "file";

/// An accepted changeset's assertions still in effect, with only the files
/// they're still in effect for.
struct InEffect {
    changeset: ChangesetDetails,
    assertions: Vec<Assertion>,
}

/// Where an assertion's retraction waits, e.g. "your draft".
pub type Pending = HashMap<AssertionUuid, String>;

/// What [`assertions_table`] offers to do with each assertion.
#[derive(Clone, Copy)]
pub enum RowAction<'a> {
    /// Nothing: a record, e.g. an interpretation's assertions, which notes
    /// what's since been retracted.
    None,
    /// Select it, to remove into the draft, unless an open changeset already
    /// retracts it, which `pending` says.
    Remove { pending: &'a Pending },
    /// Take it back out of the open changeset that makes or retracts it.
    TakeBack { changeset: ChangesetUuid },
}

impl RowAction<'_> {
    /// Whether the table is of what's in effect, rather than a record.
    fn in_effect(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// The assertions new interpretations will include, by changeset, any
/// selection of which can be removed: one, a file's, a changeset's.
#[page]
pub async fn page() -> Result<impl View> {
    Ok(view! { assertions_page(import_error: None) })
}

/// The page, with the import dialog open on `import_error` if an import
/// failed.
#[component]
async fn assertions_page(cx: &Cx, import_error: Option<&str>) -> Result<impl View> {
    let in_effect = list_in_effect(cx).await?;
    let pending = list_pending(cx).await?;

    Ok(view! {
        <div class=(SECTION_HEADING_ROW)>
            <h1 class=(SECTION_H1)>"Assertions"</h1>
            page_help(
                section: true,
                <p>
                    "Assertions are the accepted corrections of which works a file "
                    "belongs to, outranking every other claim, in every ingest. They're "
                    "grouped by the changeset that made them. To add one, mark what's "
                    "wrong while you browse, and correct it under Marked."
                </p>
                <p>
                    "Select assertions, a file's, or a whole changeset's, and remove them: "
                    "that retracts them in your draft, which applies it with your other "
                    "corrections when you accept it."
                </p>
                <p>
                    "They're the one part of what WST knows that can't be made again "
                    "from your files. Export writes them to a file, and Import makes "
                    "the assertions in such a file again, e.g. in a new instance of WST."
                </p>
            )
            <div class="ml-auto flex items-center gap-2">
                if !in_effect.is_empty() {
                    <a
                        class=(button_variants(ButtonVariant::Outline, ButtonSize::Sm))
                        href=(href!(export))
                        download=""
                        title="Download the assertions in effect as a JSON file"
                    >
                        "Export"
                    </a>
                }
                import_dialog(error: import_error)
            </div>
        </div>

        section_nav(current: SectionPage::Assertions)

        if in_effect.is_empty() {
            <p class="text-muted-foreground">"There are no assertions."</p>
        }
        // Defined in ts/groups.ts, which says how the groups' checkboxes
        // select what's in them.
        <div
            class="flex flex-col gap-6"
            x-data="groupSelection"
            x-on:change="changed($event)"
        >
            for group in &in_effect {
                changeset_assertions(group: group, pending: &pending)
            }
            remove_selected_bar()
        </div>
    })
}

/// The Import button, and the dialog it opens: a file from Export, or from
/// `wst assertions export`, which [`import`] makes a changeset of.
#[component]
async fn import_dialog(error: Option<&str>) -> Result<impl View> {
    Ok(view! {
        form_dialog(
            trigger: "Import",
            title: "Import assertions",
            description: Some(
                "Makes the assertions in a file from Export again, in a changeset \
                 of their own, which you review and accept under Drafts. Each \
                 file's data source is matched by name, so add the data sources \
                 first.",
            ),
            form_id: "import-assertions",
            submit: "Import",
            open: error.is_some(),
            <form
                id="import-assertions"
                method="post"
                enctype="multipart/form-data"
                action=(href!(import))
            >
                field_group(
                    if let Some(error) = error {
                        alert(
                            variant: AlertVariant::Destructive,
                            alert_description((error))
                        )
                    }
                    field(
                        field_label(attrs: attributes! { for="import-file" }, "File")
                        input(
                            attrs: attributes! {
                                id="import-file"
                                name=(IMPORT_FILE_FIELD)
                                type="file"
                                accept=".json,application/json"
                                required=""
                            }
                        )
                    )
                )
            </form>
        )
    })
}

/// The assertions in effect as a JSON file to download, which [`import`]
/// reads, and `wst assertions import` too.
#[route(GET "./export")]
async fn export(cx: &Cx) -> Result<Download> {
    let assertions = active_assertions(app(cx)).await?;
    let mut json = serde_json::to_vec_pretty(&assertions)?;
    json.push(b'\n');
    Ok(Download {
        name: format!(
            "wst-assertions-{}.json",
            Timestamp::now().strftime("%Y-%m-%d")
        ),
        json,
    })
}

/// A JSON file for the browser to save as `name`.
struct Download {
    name: String,
    json: Vec<u8>,
}

impl IntoResponse for Download {
    fn into_response(self, _cx: &Cx) -> Result<Response> {
        Ok(Response::builder()
            .header("Content-Type", "application/json")
            .header(
                "Content-Disposition",
                format!("attachment; filename=\"{}\"", self.name),
            )
            .body(Body::from(self.json))?)
    }
}

/// Makes the assertions in the uploaded file again, in a new changeset, and
/// shows it among the drafts. If that fails, shows this page again with the
/// import dialog open on why.
#[page(POST "./import")]
async fn import(cx: &Cx, mut multipart: Multipart) -> Result<impl View> {
    let mut upload = None;
    // Not `field`: that's the form component in scope, which a pattern would
    // match instead of binding.
    while let Some(part) = multipart.next_field().await? {
        if part.name() == Some(IMPORT_FILE_FIELD) {
            let name = part.file_name().unwrap_or("assertions.json").to_owned();
            upload = Some((name, part.text().await?));
        }
    }
    let (status, error) = match upload {
        None => (
            StatusCode::BAD_REQUEST,
            "Choose a file to import.".to_owned(),
        ),
        Some((name, contents)) => match serde_json::from_str::<v1::ActiveAssertions>(&contents) {
            Err(error) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("{name} isn't a file that Export writes: {error}"),
            ),
            Ok(v1::ActiveAssertions { assertions }) => {
                let count = match assertions.len() {
                    1 => "1 assertion".to_owned(),
                    n => format!("{n} assertions"),
                };
                let import = v1::ImportAssertions {
                    author: IMPORT_AUTHOR.to_owned(),
                    description: format!("Import {count} from {name}"),
                    assertions,
                };
                match import_exported(app(cx), import).await {
                    Ok(ImportedAssertions { changeset, .. }) => {
                        return Err(see_other(draft::changeset_href(cx, changeset)).into());
                    }
                    Err(error) => (error.status(), error.message().to_owned()),
                }
            }
        },
    };
    Ok(view! {
        (status)
        assertions_page(import_error: Some(error.as_str()))
    })
}

/// The bar that shows while any assertions are selected: how many, and a
/// button to remove them, which posts their checkboxes, `assertion` with
/// `form="remove-selected"`, to [`remove`].
#[component]
async fn remove_selected_bar() -> Result<impl View> {
    Ok(view! {
        <form
            id="remove-selected"
            method="post"
            action=(href!(remove))
            x-show="selected > 0"
            x-on:submit="if (!confirm(`Remove ${selected} selected? They go into your draft.`)) $event.preventDefault()"
            style="display: none"
            class=(SELECTED_BAR)
        >
            <span class="px-2 text-sm font-medium" x-text="`${selected} selected`"></span>
            button(
                size: ButtonSize::Sm,
                attrs: attributes! {
                    type="submit"
                    class="ml-auto"
                    title="Retract the selected assertions in your draft"
                },
                "Remove"
            )
            button(
                variant: ButtonVariant::Ghost,
                size: ButtonSize::Sm,
                attrs: attributes! { type="button" x-on:click="clear()" },
                "Clear"
            )
        </form>
    })
}

/// The accepted changesets with assertions still in effect, newest first.
async fn list_in_effect(cx: &Cx) -> Result<Vec<InEffect>> {
    let mut in_effect = Vec::new();
    for summary in queries::list_changesets(app(cx), Some(ChangesetState::Accepted)).await? {
        let Some(changeset) = queries::get_changeset(app(cx), summary.uuid).await? else {
            continue;
        };
        let assertions: Vec<Assertion> = changeset
            .assertions
            .iter()
            .filter_map(|assertion| {
                let mut assertion = assertion.clone();
                assertion.files.retain(|file| file.retracted_at.is_none());
                (!assertion.files.is_empty()).then_some(assertion)
            })
            .collect();
        if !assertions.is_empty() {
            in_effect.push(InEffect {
                changeset,
                assertions,
            });
        }
    }
    Ok(in_effect)
}

/// The assertions an open changeset retracts, by where that waits.
async fn list_pending(cx: &Cx) -> Result<Pending> {
    let mut pending = Pending::new();
    for summary in queries::list_changesets(app(cx), Some(ChangesetState::Open)).await? {
        let Some(changeset) = queries::get_changeset(app(cx), summary.uuid).await? else {
            continue;
        };
        let where_ = if changeset.author == USER {
            "your draft".to_string()
        } else {
            format!("the changeset by {}", changeset.author)
        };
        for retraction in changeset.retractions {
            pending.entry(retraction).or_insert_with(|| where_.clone());
        }
    }
    Ok(pending)
}

/// An accepted changeset: who made it and when it was accepted, its
/// assertions in effect, and a checkbox that selects them all.
#[component]
async fn changeset_assertions(cx: &Cx, group: &InEffect, pending: &Pending) -> Result<impl View> {
    let changeset = &group.changeset;
    // Whether anything is left to select.
    let selectable = group
        .assertions
        .iter()
        .any(|assertion| !pending.contains_key(&assertion.uuid));
    // Its newest interpretation: what accepting it made, if nothing changed
    // since.
    let interpretation = queries::list_changeset_interpretations(app(cx), changeset.uuid)
        .await?
        .into_iter()
        .next();
    Ok(view! {
        <section
            id=(changeset_anchor(changeset.uuid))
            class="flex flex-col gap-2"
            data-selection-scope=""
        >
            // The title, with a checkbox that selects the changeset's
            // assertions, and its interpretation at the row's end.
            <div class="flex flex-wrap items-center gap-2">
                <label class="mr-auto flex min-w-0 items-center gap-2">
                    if selectable {
                        checkbox(
                            attrs: attributes! {
                                data-selects=""
                                aria-label="Select all of its assertions"
                            }
                        )
                    }
                    <h2 class=(H2) translate="no">(changeset_title(changeset))</h2>
                </label>
                if let Some(interpretation) = &interpretation {
                    <a
                        class=(button_variants(ButtonVariant::Outline, ButtonSize::Sm))
                        href=(href!(
                            interpretation_id::page,
                            InterpretationId(interpretation.interpretation.uuid.0),
                        ))
                    >
                        "Interpretation"
                    </a>
                }
            </div>
            draft::changeset_facts(
                author: &changeset.author,
                when_label: "Accepted",
                when: changeset.closed_at.as_ref()
            )
            assertions_table(
                assertions: &group.assertions,
                action: RowAction::Remove { pending }
            )
        </section>
    })
}

/// The changeset's description, or who made it if it has none.
fn changeset_title(changeset: &ChangesetDetails) -> String {
    match changeset.description.trim() {
        "" => format!("Changeset by {}", changeset.author),
        description => description.to_string(),
    }
}

/// The `id` of a changeset's section, to link to it.
pub fn changeset_anchor(changeset: ChangesetUuid) -> String {
    format!("changeset-{changeset}")
}

/// A link to an accepted changeset's assertions in effect.
pub fn changeset_href(cx: &Cx, changeset: ChangesetUuid) -> String {
    format!(
        "{}#{}",
        href!(page).resolve(cx),
        changeset_anchor(changeset)
    )
}

/// Retracts the selected assertions, the form's `assertion` fields, in the
/// draft, but for any an open changeset already retracts, then shows the
/// assertions again.
#[route(POST "./remove")]
async fn remove(cx: &Cx, RawForm(body): RawForm) -> Result<SeeOther> {
    let selected: Vec<AssertionUuid> = form_urlencoded::parse(&body)
        .filter(|(key, _)| key == "assertion")
        .filter_map(|(_, value)| value.parse().ok().map(AssertionUuid))
        .collect();
    if !selected.is_empty() {
        let pending = list_pending(cx).await?;
        let draft = commands::open_draft(app(cx))
            .await
            .map_err(|e| PageError::log(e.into()))?;
        for assertion in selected {
            if pending.contains_key(&assertion) {
                continue;
            }
            commands::retract_in_changeset(app(cx), draft, assertion)
                .await
                .map_err(|e| PageError::log(e.into()))?;
        }
    }
    Ok(see_other(href!(page).resolve(cx)))
}

/// Assertions, newest first, those about the same files sharing a row, each
/// claim with the button or checkbox `action` gives it.
/// Also the drafts' changes, and an interpretation's assertions.
#[component]
pub async fn assertions_table(
    assertions: &[Assertion],
    action: RowAction<'_>,
) -> Result<impl View> {
    let groups = group_by_files(assertions, action.in_effect());
    // Whether a row's files get a checkbox that selects its assertions.
    let selecting = matches!(action, RowAction::Remove { .. });

    Ok(view! {
        table(
            attrs: attributes! { class="stacked-table" },
            table_header(
                table_row(
                    table_head("Files")
                    table_head("Claims")
                )
            )
            table_body(
                // The files take half the width, the claims and buttons the
                // rest. A claim wraps, as one naming a path, e.g. `Extra of:
                // …`, is long.
                for (files, assertions) in groups {
                    // Whether any of the row's assertions can be selected.
                    let selectable = match action {
                        RowAction::Remove { pending } => assertions
                            .iter()
                            .any(|assertion| !pending.contains_key(&assertion.uuid)),
                        _ => false,
                    };
                    table_row(
                        attrs: attributes! { data-selection-scope=(selecting.then_some("")) },
                        table_cell(
                            attrs: attributes! { class="whitespace-normal! sm:w-1/2" },
                            // The row's checkbox, before its files, selects all
                            // of its assertions.
                            <div class="flex items-start gap-2">
                                if selectable {
                                    checkbox(
                                        attrs: attributes! {
                                            class="mt-0.5"
                                            data-selects=""
                                            aria-label="Select all assertions about these files"
                                        }
                                    )
                                }
                                <div class="min-w-0 flex-1">path_list(files: files)</div>
                            </div>
                        )
                        table_cell(
                            attrs: attributes! { class="whitespace-normal! sm:w-1/2" },
                            // Each claim beside its button or note.
                            <div
                                class="grid w-full grid-cols-[minmax(0,1fr)_auto] items-center gap-x-3 gap-y-1"
                            >
                                for assertion in assertions {
                                    <div class="[overflow-wrap:anywhere]">
                                        display::claim_pred(pred: &assertion.claim)
                                    </div>
                                    match action {
                                        RowAction::None => {
                                            <div
                                                class="justify-self-end text-sm text-muted-foreground"
                                            >
                                                if assertion.retracted_at().is_some() {
                                                    "Retracted"
                                                }
                                            </div>
                                        }
                                        RowAction::Remove { pending } => {
                                            match pending.get(&assertion.uuid) {
                                                Some(where_) => {
                                                    <div
                                                        class="justify-self-end text-sm text-muted-foreground"
                                                    >
                                                        (format!("Retracted in {where_}"))
                                                    </div>
                                                }
                                                None => {
                                                    checkbox(
                                                        attrs: attributes! {
                                                            class="justify-self-end"
                                                            form="remove-selected"
                                                            name="assertion"
                                                            value=(assertion.uuid.to_string())
                                                            aria-label="Select this assertion"
                                                        }
                                                    )
                                                }
                                            }
                                        }
                                        RowAction::TakeBack { changeset } => {
                                            <form
                                                class="justify-self-end"
                                                method="post"
                                                action=(href!(draft::take_back))
                                            >
                                                <input
                                                    type="hidden"
                                                    name="changeset"
                                                    value=(changeset.to_string())
                                                >
                                                <input
                                                    type="hidden"
                                                    name="assertion"
                                                    value=(assertion.uuid.to_string())
                                                >
                                                button(
                                                    variant: ButtonVariant::Outline,
                                                    size: ButtonSize::Sm,
                                                    attrs: attributes! { type="submit" },
                                                    "Take back"
                                                )
                                            </form>
                                        }
                                    }
                                }
                            </div>
                        )
                    )
                }
            )
        )
    })
}

/// Assertions grouped by the files they list, in the order each group's first
/// assertion comes. `in_effect` is as for [`listed_files`].
fn group_by_files(
    assertions: &[Assertion],
    in_effect: bool,
) -> Vec<(Vec<ListedFile>, Vec<&Assertion>)> {
    let mut groups: Vec<(Vec<ListedFile>, Vec<&Assertion>)> = Vec::new();
    for assertion in assertions {
        let mut files = listed_files(assertion, in_effect);
        files.sort();
        match groups
            .iter_mut()
            .find(|(group_files, _)| *group_files == files)
        {
            Some((_, group)) => group.push(assertion),
            None => groups.push((files, vec![assertion])),
        }
    }
    groups
}

/// An assertion's files, to list as a tree of their folders. A record of
/// assertions, rather than those `in_effect`, notes a file it was retracted
/// from while it stands for the others.
fn listed_files(assertion: &Assertion, in_effect: bool) -> Vec<ListedFile> {
    assertion
        .files
        .iter()
        .map(|file| ListedFile {
            data_source_name: file.data_source_name.clone(),
            path: file.rel_path.display().to_string(),
            // Replaced by a newer assertion for this file only.
            note: (!in_effect && file.retracted_at.is_some() && assertion.retracted_at().is_none())
                .then(|| "retracted".to_string()),
        })
        .collect()
}

//! The drafts: the open changesets, yours and those agents propose, each
//! reviewed as a catalog diff, then accepted or discarded. The second page of
//! the Corrections section (see `app::corrections`).

use crate::app::assertions::{RowAction, assertions_table};
use crate::app::corrections::{SectionPage, section_nav};
use crate::app::interpretations::{canonical_badge, interpretation_state};
use crate::app::page_help::page_help;
use crate::{
    app::interpretations::interpretation_id::{self, InterpretationId},
    app::movies::movie_id::{self, MovieId},
    app::tv_series::series_id::{self, SeriesId},
    context::app,
    display,
    error::PageError,
    styles::{FACT_LABEL, FACTS, H2, LINK, MONO, SECTION_H1, SECTION_HEADING_ROW},
    ui::{
        alert::{AlertVariant, alert, alert_description},
        badge::{BadgeVariant, badge},
        button::{ButtonVariant, button},
    },
};
use jiff::Timestamp;
use serde::Deserialize;
use std::time::Duration;
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
use wanshitong_app::domain::PathId;
use wanshitong_app::{
    commands::{self, USER},
    domain::{
        Assertion, AssertionUuid, CatalogFile, CatalogWork, CatalogWorkKind, ChangesetDetails,
        ChangesetState, ChangesetUuid, FilePart, InterpretationUuid,
    },
    interpretations::{InterpretationState, InterpretationStatus},
    queries::{self, InterpretationDiff, WorkChange, WorkChangeKind},
};

/// The open changesets, yours first: each one's changes, the assertions it
/// makes and those it retracts, with buttons to take one back, and to accept
/// or discard the changeset.
#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let open = list_open(cx).await?;
    // The accepted assertions, to say what the retracted ones were.
    let active = queries::list_active_assertions(app(cx)).await?;

    Ok(view! {
        <div class=(SECTION_HEADING_ROW)>
            <h1 class=(SECTION_H1)>"Drafts"</h1>
            page_help(
                section: true,
                <p>
                    "Corrections go into drafts: yours, and the changesets agents propose. "
                    "A draft changes nothing until you accept it, which applies all of its "
                    "corrections together, with a new interpretation that becomes canonical "
                    "when it finishes."
                </p>
                <p>
                    "Taking a correction back drops it from its draft. Discarding a draft "
                    "drops all of them."
                </p>
            )
        </div>

        section_nav(current: SectionPage::Drafts)

        if open.is_empty() {
            <p class="text-muted-foreground">
                "There's no draft. Make a correction to start one."
            </p>
        }
        // Keyed, as each section may hold a live region: the changeset's diff.
        #[key(changeset.uuid.0.as_bytes())]
        for changeset in &open {
            changeset_section(changeset: changeset, active: &active)
        }
    })
}

/// The open changesets, with their changes: yours first, then the rest newest
/// first.
async fn list_open(cx: &Cx) -> Result<Vec<ChangesetDetails>> {
    let mut open = Vec::new();
    for summary in queries::list_changesets(app(cx), Some(ChangesetState::Open)).await? {
        if let Some(changeset) = queries::get_changeset(app(cx), summary.uuid).await? {
            open.push(changeset);
        }
    }
    open.sort_by_key(|changeset| changeset.author != USER);
    Ok(open)
}

/// Whether a changeset is the draft the web UI's corrections go into.
fn is_yours(changeset: &ChangesetDetails) -> bool {
    changeset.author == USER
}

/// The changeset's description, or who made it if it has none.
fn changeset_title(changeset: &ChangesetDetails) -> String {
    match changeset.description.trim() {
        "" => format!("Changeset by {}", changeset.author),
        description => description.to_string(),
    }
}

/// A link to an open changeset on this page.
pub fn changeset_href(cx: &Cx, changeset: ChangesetUuid) -> String {
    format!("{}#changeset-{changeset}", href!(page).resolve(cx))
}

/// Who made a changeset, and when it was started or accepted, each on a
/// line of its own: an author's name can have spaces, e.g. an agent run's,
/// so it isn't run into the rest. Also under an accepted changeset's
/// assertions.
#[component]
pub async fn changeset_facts(
    author: &str,
    when_label: &str,
    when: Option<&Timestamp>,
) -> Result<impl View> {
    Ok(view! {
        <dl class=(FACTS)>
            <dt class=(FACT_LABEL)>"Author"</dt>
            <dd class="[overflow-wrap:anywhere]" translate="no">(author)</dd>
            if let Some(when) = when {
                <dt class=(FACT_LABEL)>(when_label)</dt>
                <dd class=(MONO) translate="no">(display::ts(when).to_string())</dd>
            }
        </dl>
    })
}

/// One open changeset: its changes, a preview of what accepting it changes,
/// its interpretations, and buttons to accept or discard it.
#[component]
async fn changeset_section(
    cx: &Cx,
    changeset: &ChangesetDetails,
    active: &[Assertion],
) -> Result<impl View> {
    let yours = is_yours(changeset);
    let made: Vec<Assertion> = changeset
        .assertions
        .iter()
        .filter(|assertion| assertion.retracted_at().is_none())
        .cloned()
        .collect();
    let retracted: Vec<Assertion> = active
        .iter()
        .filter(|assertion| changeset.retractions.contains(&assertion.uuid))
        .cloned()
        .collect();
    // Only an interpretation that's up to date shows by itself; making one
    // takes a while, so it waits to be asked for.
    let current = commands::current_changeset_interpretation(app(cx), changeset.uuid)
        .await
        .map_err(|e| PageError::log(e.into()))?;
    let interpretations = queries::list_changeset_interpretations(app(cx), changeset.uuid).await?;
    let (accept_confirm, discard_confirm) = if yours {
        (
            "Accept your draft? A new interpretation will apply it, and become canonical when it finishes.",
            "Discard your draft, and every correction in it?",
        )
    } else {
        (
            "Accept this changeset? A new interpretation will apply it, and become canonical when it finishes.",
            "Discard this changeset, and every correction in it?",
        )
    };

    Ok(view! {
        <section
            id=(format!("changeset-{}", changeset.uuid))
            class="flex flex-col gap-4"
        >
            <div class="flex flex-col gap-2">
                <h2 class=(H2)>
                    if yours {
                        "Your draft"
                    } else {
                        <span translate="no">(changeset_title(changeset))</span>
                    }
                </h2>
                changeset_facts(
                    author: &changeset.author,
                    when_label: "Started",
                    when: Some(&changeset.created_at)
                )
            </div>

            if made.is_empty() && retracted.is_empty() {
                <p class="text-muted-foreground">"It has no corrections yet."</p>
            }

            if !made.is_empty() {
                changes_table(changeset: changeset.uuid, heading: "Asserts", assertions: &made)
            }
            if !retracted.is_empty() {
                changes_table(changeset: changeset.uuid, heading: "Retracts", assertions: &retracted)
            }

            if !made.is_empty() || !retracted.is_empty() {
                <section class="flex flex-col gap-2">
                    <h3 class="font-semibold">"Preview changes"</h3>
                    match current {
                        Some(interpretation) => {
                            draft_diff(interpretation_id: interpretation.to_string())
                        }
                        None => {
                            <form
                                class="flex flex-wrap items-center gap-3"
                                method="post"
                                action=(href!(interpret))
                            >
                                <input
                                    type="hidden"
                                    name="changeset"
                                    value=(changeset.uuid.to_string())
                                >
                                button(
                                    variant: ButtonVariant::Outline,
                                    attrs: attributes! { type="submit" },
                                    "Show what changes"
                                )
                                <p class="text-sm text-muted-foreground">
                                    "Interprets the library with these corrections, which takes a while."
                                </p>
                            </form>
                        }
                    }
                </section>
            }

            if !interpretations.is_empty() {
                changeset_interpretations(interpretations: &interpretations, current: current)
            }

            <div class="flex flex-wrap gap-2">
                <form
                    method="post"
                    action=(href!(accept))
                    data-confirm=(accept_confirm)
                    onsubmit="return confirm(this.dataset.confirm)"
                >
                    <input type="hidden" name="changeset" value=(changeset.uuid.to_string())>
                    button(attrs: attributes! { type="submit" }, "Accept")
                </form>
                <form
                    method="post"
                    action=(href!(discard))
                    data-confirm=(discard_confirm)
                    onsubmit="return confirm(this.dataset.confirm)"
                >
                    <input type="hidden" name="changeset" value=(changeset.uuid.to_string())>
                    button(
                        variant: ButtonVariant::Outline,
                        attrs: attributes! { type="submit" },
                        "Discard"
                    )
                </form>
            </div>
        </section>
    })
}

/// Assertions a changeset makes, or retracts, each with a button to take it
/// back: the assertions table, which lays claims out beside their files.
#[component]
async fn changes_table(
    changeset: ChangesetUuid,
    heading: &str,
    assertions: &[Assertion],
) -> Result<impl View> {
    Ok(view! {
        <section class="flex flex-col gap-2">
            <h3 class="font-semibold">(heading)</h3>
            assertions_table(
                assertions: assertions,
                action: RowAction::TakeBack { changeset }
            )
        </section>
    })
}

/// The interpretations made of a changeset, to preview it: the current one,
/// if any, and the stale ones collapsed.
#[component]
async fn changeset_interpretations(
    interpretations: &[InterpretationStatus],
    current: Option<InterpretationUuid>,
) -> Result<impl View> {
    let (current, stale): (Vec<&InterpretationStatus>, Vec<&InterpretationStatus>) =
        interpretations
            .iter()
            .partition(|details| Some(details.interpretation.uuid) == current);
    Ok(view! {
        <section class="flex flex-col gap-2">
            <h3 class="font-semibold">"Interpretations"</h3>
            if !current.is_empty() {
                <ul class="flex flex-col gap-1 text-sm">
                    for details in &current {
                        interpretation_item(details: details, current: true)
                    }
                </ul>
            }
            if !stale.is_empty() {
                <details class="text-sm">
                    <summary class="cursor-pointer text-muted-foreground">
                        (format!("Stale ({})", stale.len()))
                    </summary>
                    <ul class="mt-2 flex flex-col gap-1">
                        for details in &stale {
                            interpretation_item(details: details, current: false)
                        }
                    </ul>
                </details>
            }
        </section>
    })
}

/// An interpretation of a changeset: a link to it, its state, and when it
/// started. The current one is what accepting the changeset now would reuse.
#[component]
async fn interpretation_item(details: &InterpretationStatus, current: bool) -> Result<impl View> {
    let interpretation = &details.interpretation;
    Ok(view! {
        <li class="flex flex-wrap items-center gap-2">
            <a
                class=(LINK)
                href=(href!(
                    interpretation_id::page,
                    InterpretationId(interpretation.uuid.0),
                ))
                translate="no"
            >
                (interpretation.name.as_str())
            </a>
            interpretation_state(details: details)
            if interpretation.is_canonical {
                canonical_badge()
            }
            if current {
                badge(variant: BadgeVariant::Outline, "Current")
            }
            <span class=(MONO) translate="no">
                (display::ts(&interpretation.started_at).to_string())
            </span>
        </li>
    })
}

/// Shown on every page while there's a draft, or a changeset awaits review:
/// how many corrections your draft has, how many changesets wait, and a link
/// to review them.
#[component]
pub async fn draft_bar(cx: &Cx) -> Result<impl View> {
    let open = queries::list_changesets(app(cx), Some(ChangesetState::Open)).await?;
    let yours = match open.iter().find(|changeset| changeset.author == USER) {
        Some(summary) => queries::get_changeset(app(cx), summary.uuid).await?,
        None => None,
    };
    let changes = yours.as_ref().map(|draft| {
        let assertions = draft
            .assertions
            .iter()
            .filter(|assertion| assertion.retracted_at().is_none())
            .count();
        assertions + draft.retractions.len()
    });
    let others = open
        .iter()
        .filter(|changeset| changeset.author != USER)
        .count();
    let yours_text = changes.map(|changes| match changes {
        1 => "Your draft has 1 correction".to_string(),
        n => format!("Your draft has {n} corrections"),
    });
    let others_text = match others {
        0 => None,
        1 => Some("1 changeset awaits review".to_string()),
        n => Some(format!("{n} changesets await review")),
    };
    let text = match (yours_text, others_text) {
        (None, None) => None,
        (Some(yours), None) => Some(format!("{yours}.")),
        (None, Some(others)) => Some(format!("{others}.")),
        (Some(yours), Some(others)) => Some(format!("{yours}, and {others}.")),
    };
    Ok(view! {
        if let Some(text) = text {
            <div class="border-b border-border bg-foreground/5">
                <div
                    class="mx-auto flex max-w-5xl items-center gap-2 px-6 py-1.5 text-sm"
                >
                    <span>(text)</span>
                    <a class=(LINK) href=(href!(page))>"Review"</a>
                </div>
            </div>
        }
    })
}

#[derive(Deserialize)]
struct ChangesetForm {
    changeset: ChangesetUuid,
}

/// Starts interpreting a changeset, to show what accepting it changes, then
/// shows the drafts again.
#[route(POST "./interpret")]
async fn interpret(cx: &Cx, Form(form): Form<ChangesetForm>) -> Result<SeeOther> {
    commands::start_changeset_interpretation(app(cx), form.changeset)
        .await
        .map_err(|e| PageError::log(e.into()))?;
    Ok(see_other(changeset_href(cx, form.changeset)))
}

/// Accepts a changeset, then shows the interpretation that applies it.
#[route(POST "./accept")]
async fn accept(cx: &Cx, Form(form): Form<ChangesetForm>) -> Result<SeeOther> {
    let interpretation = commands::accept_changeset(app(cx), form.changeset)
        .await
        .map_err(|e| PageError::log(e.into()))?;
    Ok(see_other(
        href!(interpretation_id::page, InterpretationId(interpretation.0)).resolve(cx),
    ))
}

/// Discards a changeset.
#[route(POST "./discard")]
async fn discard(cx: &Cx, Form(form): Form<ChangesetForm>) -> Result<SeeOther> {
    commands::discard_changeset(app(cx), form.changeset)
        .await
        .map_err(|e| PageError::log(e.into()))?;
    Ok(see_other(href!(page).resolve(cx)))
}

#[derive(Deserialize)]
struct TakeBackForm {
    changeset: ChangesetUuid,
    assertion: AssertionUuid,
}

/// Takes a correction back out of a changeset: an assertion it makes, or a
/// retraction.
#[route(POST "./take-back")]
pub async fn take_back(cx: &Cx, Form(form): Form<TakeBackForm>) -> Result<SeeOther> {
    commands::take_back_from_changeset(app(cx), form.changeset, form.assertion)
        .await
        .map_err(|e| PageError::log(e.into()))?;
    Ok(see_other(changeset_href(cx, form.changeset)))
}

/// A notice of what the draft asserts about any of `files`, if anything, e.g.
/// on a movie's page after correcting it.
#[component]
pub async fn pending_in_draft(cx: &Cx, files: Vec<PathId>) -> Result<impl View> {
    let draft = match commands::find_draft(app(cx)).await? {
        Some(uuid) => queries::get_changeset(app(cx), uuid).await?,
        None => None,
    };
    let claims: Vec<_> = draft
        .iter()
        .flat_map(|draft| &draft.assertions)
        .filter(|assertion| {
            assertion.files.iter().any(|file| {
                file.retracted_at.is_none()
                    && files.contains(&PathId::new(file.data_source_uuid, &file.rel_path))
            })
        })
        .map(|assertion| assertion.claim.clone())
        .collect();
    Ok(view! {
        if !claims.is_empty() {
            alert(
                alert_description(
                    "Your draft corrects this: "
                    for (i, claim) in claims.iter().enumerate() {
                        if i > 0 {
                            "; "
                        }
                        display::claim_pred(pred: claim)
                    }
                    ". "
                    <a class=(LINK) href=(href!(page))>"Review it"</a>
                    " to apply it with your other corrections."
                )
            )
        }
    })
}

/// How often the diff re-checks the changeset's interpretation while it runs.
const REFRESH: Duration = Duration::from_secs(1);

/// What accepting a changeset would change in the catalog, from its up-to-date
/// interpretation, as a live region that shows the diff once the
/// interpretation finishes.
///
/// `interpretation_id` comes from the browser, as a string, like any shard
/// argument.
#[shard]
async fn draft_diff(cx: &Cx, interpretation_id: String) -> Result<impl View> {
    let interpretation = InterpretationUuid(interpretation_id.parse().ok().ok_or_not_found()?);

    Ok(view! {
        <div class="flex flex-col gap-3">
            (live! {
                loop {
                    let details = queries::get_interpretation(app(cx), interpretation).await?.ok_or_not_found(

                    )?;
                    if details.state == InterpretationState::Running && connected(cx) {
                        emit! {
                            <p class="text-sm text-muted-foreground" role="status">
                                "Interpreting the changeset, to see what accepting it changes…"
                            </p>
                        }?;
                        tokio::time::sleep(REFRESH).await;
                        continue;
                    }
                    let diff = match details.state {
                        InterpretationState::Succeeded => Some(
                            queries::diff_interpretations(app(cx), None, interpretation).await.map_err(
                                |e| PageError::log(e.into()),
                            )?,
                        ),
                        _ => None,
                    };
                    break emit! { diff_result(details: &details, diff: diff.as_ref()) };
                }
            })
        </div>
    })
}

#[component]
async fn diff_result(
    details: &InterpretationStatus,
    diff: Option<&InterpretationDiff>,
) -> Result<impl View> {
    let interpretation_href = href!(
        interpretation_id::page,
        InterpretationId(details.interpretation.uuid.0)
    );
    Ok(view! {
        match (details.state, diff) {
            (InterpretationState::Running, _) => {
                <p class="text-sm text-muted-foreground">
                    "The changeset is still being interpreted. "
                    "Reload the page to see what accepting it changes."
                </p>
            }
            (_, Some(diff)) => {
                if diff.changes.is_empty() && diff.unidentified.is_empty() {
                    <p class="text-sm text-muted-foreground">
                        "Accepting it changes nothing in the catalog."
                    </p>
                }
                if !diff.changes.is_empty() {
                    <p class="text-sm">(diff_summary(&diff.changes))</p>
                    <ul class="flex flex-col gap-3">
                        for change in &diff.changes {
                            work_change(change: change)
                        }
                    </ul>
                }
                unidentified_files(
                    heading: "No longer unidentified",
                    files: &diff.unidentified.left
                )
                unidentified_files(
                    heading: "Newly unidentified",
                    files: &diff.unidentified.joined
                )
            }
            (_, None) => {
                alert(
                    variant: AlertVariant::Destructive,
                    alert_description(
                        "Interpreting the changeset didn't finish, so there's nothing to compare. "
                        <a class=(LINK) href=(interpretation_href)>
                            "See its interpretation"
                        </a>
                        " for why."
                    )
                )
            }
        }
    })
}

/// E.g. "3 changes: 2 removed, 1 changed".
fn diff_summary(changes: &[WorkChange]) -> String {
    let mut kinds: Vec<(WorkChangeKind, usize)> = Vec::new();
    for change in changes {
        match kinds.iter_mut().find(|(kind, _)| *kind == change.kind()) {
            Some((_, n)) => *n += 1,
            None => kinds.push((change.kind(), 1)),
        }
    }
    let kinds: Vec<_> = kinds
        .into_iter()
        .map(|(kind, n)| format!("{n} {}", kind_text(kind)))
        .collect();
    let changes = match changes.len() {
        1 => "1 change".to_string(),
        n => format!("{n} changes"),
    };
    format!("{changes}: {}", kinds.join(", "))
}

fn kind_text(kind: WorkChangeKind) -> &'static str {
    match kind {
        WorkChangeKind::Added => "added",
        WorkChangeKind::Removed => "removed",
        WorkChangeKind::Changed => "changed",
        WorkChangeKind::Split => "split",
        WorkChangeKind::Merged => "merged",
        WorkChangeKind::Regrouped => "regrouped",
    }
}

/// One change: the works before and after, and how each of their files'
/// placements differs.
#[component]
async fn work_change(change: &WorkChange) -> Result<impl View> {
    let kind = change.kind();
    let variant = match kind {
        WorkChangeKind::Added => BadgeVariant::Success,
        WorkChangeKind::Removed => BadgeVariant::Destructive,
        _ => BadgeVariant::Secondary,
    };
    let label = match kind {
        WorkChangeKind::Added => "Added",
        WorkChangeKind::Removed => "Removed",
        WorkChangeKind::Changed if change.only_files_changed() => "Files changed",
        WorkChangeKind::Changed => "Changed",
        WorkChangeKind::Split => "Split",
        WorkChangeKind::Merged => "Merged",
        WorkChangeKind::Regrouped => "Regrouped",
    };
    // The same work on both sides needs showing once.
    let after: &[CatalogWork] = if change.only_files_changed() {
        &[]
    } else {
        &change.after
    };
    Ok(view! {
        <li class="flex flex-col gap-2 rounded-lg border border-border p-3">
            <div class="flex flex-wrap items-center gap-2">
                badge(variant: variant, (label))
                if !change.before.is_empty() {
                    work_links(of: &change.before)
                }
                if !change.before.is_empty() && !after.is_empty() {
                    <span class="text-muted-foreground" aria-label="becomes">
                        "→"
                    </span>
                }
                if !after.is_empty() {
                    work_links(of: after)
                }
            </div>
            <ul class="flex flex-col gap-2 text-sm">
                for file in &change.files {
                    <li class="flex min-w-0 flex-col">
                        <span
                            class=(format!("{MONO} [overflow-wrap:anywhere]"))
                            translate="no"
                        >
                            (format!(
                                "{}: {}",
                                file.data_source_name,
                                file.rel_path.display(),
                            ))
                        </span>
                        <span class="text-muted-foreground">
                            "− "
                            file_placements(
                                placements: &file.before,
                                of: &change.before
                            )
                        </span>
                        <span>
                            "+ "
                            file_placements(placements: &file.after, of: &change.after)
                        </span>
                    </li>
                }
            </ul>
            if change.disc_files > 0 {
                <p class="text-sm text-muted-foreground">
                    (match change.disc_files {
                        1 => "and 1 disc file".to_string(),
                        n => format!("and {n} disc files"),
                    })
                </p>
            }
        </li>
    })
}

/// Works, each linked to its page, e.g. `Movie Heat (1995)`.
#[component]
async fn work_links(of: &[CatalogWork]) -> Result<impl View> {
    Ok(view! {
        <span class="flex flex-wrap items-center gap-x-2 font-medium">
            for work in of {
                match work.kind {
                    CatalogWorkKind::Movie => {
                        <a
                            class=(LINK)
                            href=(href!(movie_id::page, MovieId(work.uuid)))
                        >
                            work_title(work: work)
                        </a>
                    }
                    CatalogWorkKind::TvSeries => {
                        <a
                            class=(LINK)
                            href=(href!(series_id::page, SeriesId(work.uuid)))
                        >
                            work_title(work: work)
                        </a>
                    }
                }
            }
        </span>
    })
}

/// E.g. `Movie Heat (1995)`, or `Series The Wire`.
#[component]
async fn work_title(work: &CatalogWork) -> Result<impl View> {
    let kind = match work.kind {
        CatalogWorkKind::Movie => "Movie",
        CatalogWorkKind::TvSeries => "Series",
    };
    Ok(view! {
        (kind)
        " "
        <span translate="no">(work.title.as_str())</span>
        if let Some(year) = work.year {
            (format!(" ({year})"))
        }
    })
}

/// What a file is of the works, e.g. `main file of Heat`, or `in no work`.
#[component]
async fn file_placements(
    placements: &[(usize, FilePart)],
    of: &[CatalogWork],
) -> Result<impl View> {
    let parts = placements.iter().map(|(work, part)| {
        let part = match part {
            FilePart::Movie { role, .. } => display::movie_part(*role).to_string(),
            FilePart::Episode { season, episode } => display::episode_number(*season, *episode),
            FilePart::SeriesExtra => "extra".to_string(),
            FilePart::SeriesDisc => "disc".to_string(),
        };
        (part, of[*work].title.as_str())
    });
    Ok(view! {
        if placements.is_empty() {
            "in no work"
        }
        for (i, (part, title)) in parts.enumerate() {
            if i > 0 {
                "; "
            }
            (format!("{part} of "))
            <span translate="no">(title)</span>
        }
    })
}

/// Video files that leave or join the unidentified videos without a work
/// changing, e.g. excluded ones, if there are any.
#[component]
async fn unidentified_files(heading: &str, files: &[CatalogFile]) -> Result<impl View> {
    Ok(view! {
        if !files.is_empty() {
            <div class="flex flex-col gap-2 rounded-lg border border-border p-3">
                <p class="font-medium">(format!("{heading} ({})", files.len()))</p>
                <ul class="flex flex-col gap-1 text-sm">
                    for file in files {
                        <li
                            class=(format!("{MONO} [overflow-wrap:anywhere]"))
                            translate="no"
                        >
                            (format!(
                                "{}: {}",
                                file.data_source_name,
                                file.rel_path.display(),
                            ))
                        </li>
                    }
                </ul>
            </div>
        }
    })
}

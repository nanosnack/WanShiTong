//! Marking things as needing correction while browsing, and correcting what's
//! marked.
//!
//! This module is the Corrections page: what's marked, as cards of files with
//! checkboxes. [`marking`] has the marks themselves, made from other pages, and
//! [`identify`] the panel that corrects the selected files.
//!
//! It's the front page of the Corrections section, whose other pages are the
//! drafts (`app::draft`) and the assertions in effect (`app::assertions`), in
//! the order a correction lives through: noticed, made, accepted. Each shows
//! [`section_nav`].

pub mod identify;
pub mod marking;

use crate::app::file_key::FileKey;
use crate::app::files::disc_icon;
use crate::app::files::source_id::{
    SourceId,
    rel_path::{self, RelPath},
};
use crate::app::page_help::page_help;
use crate::app::path_tree::{PathRow, indent, listed_path, path_order, path_rows};
use crate::app::{assertions, draft};
use crate::{
    app::tmdb_id::TmdbKind,
    context::app,
    display,
    error::PageError,
    icons::{FOLDER, lucide},
    styles::{LINK, SECTION_H1, SECTION_HEADING_ROW},
    ui::{
        button::{ButtonSize, ButtonVariant, button, button_variants},
        checkbox::checkbox,
    },
};
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{
        content::Form,
        error::{RouterErrorExt, SeeOther, see_other},
        href, page, query_params, route,
    },
    view::{View, attributes, component, view},
};
use wanshitong_app::domain::PathId;
use wanshitong_app::{
    commands,
    domain::{ClaimPredicate, FileDetails, Mark, MarkProgress, MarkUuid, MarkedFile},
    queries,
};

/// A search for the work to identify the selected files as.
#[query_params(error = bad_request)]
struct Search {
    query: Option<String>,
    kind: Option<TmdbKind>,
    year: Option<u32>,
}

/// What's marked as needing correction, newest first, each file with a
/// checkbox, beside the Identify panel, which corrects the selected files.
#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let Search { query, kind, year } = query_params::<Search>(cx)?;
    let kind = kind.unwrap_or_default();
    let marks = queries::list_marks(app(cx)).await?;
    let (done, marks): (Vec<_>, Vec<_>) = marks
        .into_iter()
        .partition(|mark| mark.progress == MarkProgress::Done);
    let here = href!(page).resolve(cx);
    let results = match query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        Some(query) => Some(identify::search(cx, kind, query, *year).await?),
        None => None,
    };

    Ok(view! {
        <div class=(SECTION_HEADING_ROW)>
            <h1 class=(SECTION_H1)>"Corrections"</h1>
            page_help(
                section: true,
                <p>
                    "Everything marked as needing correction. Mark a movie, a series or a "
                    "file while you browse, then correct them here together."
                </p>
                <p>
                    "Select files, and identify what they are in the Identify panel. "
                    "Corrections go into your draft. A mark is done once accepted "
                    "corrections cover all of its files."
                </p>
            )
        </div>

        section_nav(current: SectionPage::Marked)

        <div class="grid gap-6 lg:grid-cols-[1fr_22rem]">
            <div class="flex min-w-0 flex-col gap-3">
                if marks.is_empty() {
                    <p class="text-muted-foreground">
                        "Nothing needs correcting. "
                        "Mark a movie, series, or file for it to show up here."
                    </p>
                }
                for mark in &marks {
                    mark_card(mark: mark, return_to: here.as_str())
                }

                if !done.is_empty() {
                    <details class="text-sm">
                        <summary class="cursor-pointer text-muted-foreground">
                            (format!("Done ({})", done.len()))
                        </summary>
                        <div class="mt-3 flex flex-col gap-3">
                            for mark in &done {
                                mark_card(mark: mark, return_to: here.as_str())
                            }
                        </div>
                    </details>
                }
            </div>

            identify::identify_panel(
                query: query.as_deref().unwrap_or_default(),
                kind: kind,
                year: *year,
                results: results.as_ref()
            )
        </div>
    })
}

/// The pages of the Corrections section.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SectionPage {
    /// What's marked as needing correction: this module's page.
    Marked,
    /// The open changesets, to review and accept.
    Drafts,
    /// The accepted assertions in effect.
    Assertions,
}

/// Links to the section's pages, the current one highlighted. Shown under each
/// page's heading.
#[component]
pub async fn section_nav(cx: &Cx, current: SectionPage) -> Result<impl View> {
    let pages = [
        (SectionPage::Marked, "Marked", href!(page).resolve(cx)),
        (
            SectionPage::Drafts,
            "Drafts",
            href!(draft::page).resolve(cx),
        ),
        (
            SectionPage::Assertions,
            "Assertions",
            href!(assertions::page).resolve(cx),
        ),
    ];
    Ok(view! {
        <nav class="flex gap-1" aria-label="Corrections">
            for (option, label, url) in &pages {
                <a
                    href=(url.as_str())
                    aria-current=((*option == current).then_some("page"))
                    class=(button_variants(
                        if *option == current {
                            ButtonVariant::Secondary
                        } else {
                            ButtonVariant::Ghost
                        },
                        ButtonSize::Sm,
                    ))
                >
                    (*label)
                </a>
            }
        </nav>
    })
}

/// A marked file, as its mark's card lists it.
struct CardFile<'a> {
    file: &'a MarkedFile,
    /// Where it's listed: a disc where its files are, e.g. `Film/BDMV`.
    listed: String,
    /// As the canonical interpretation sees it, or None if it isn't in the
    /// latest ingests.
    details: Option<FileDetails>,
}

impl CardFile<'_> {
    fn is_disc(&self) -> bool {
        self.details
            .as_ref()
            .is_some_and(|details| details.disc.is_some())
    }
}

/// A marked thing's files, each with a checkbox to select it for the Identify
/// panel, what it is in the catalog now, and its duration.
#[component]
async fn mark_card(cx: &Cx, mark: &Mark, return_to: &str) -> Result<impl View> {
    let mut files = Vec::with_capacity(mark.files.len());
    for file in &mark.files {
        let details = queries::get_file(app(cx), file.data_source_uuid, &file.rel_path).await?;
        // Where it's listed: a disc where its files are, e.g. `Film/BDMV`.
        let path = file.rel_path.display().to_string();
        let is_disc = details
            .as_ref()
            .is_some_and(|details| details.disc.is_some());
        files.push(CardFile {
            file,
            listed: listed_path(&path, is_disc).to_string(),
            details,
        });
    }
    files.sort_by(|a, b| {
        path_order(
            (&a.file.data_source_name, &a.listed),
            (&b.file.data_source_name, &b.listed),
        )
    });
    let rows = {
        let paths: Vec<(&str, &str)> = files
            .iter()
            .map(|file| (file.file.data_source_name.as_str(), file.listed.as_str()))
            .collect();
        path_rows(&paths, None)
    };
    Ok(view! {
        // A folder's checkbox selects the files under it, and Select all
        // selects them all; see `cascadeRows` and `selectRows` in ts/tree.ts.
        <section
            class="flex flex-col gap-2 rounded-lg border border-border p-4"
            x-data=""
            x-on:change="cascadeRows($event)"
        >
            <div class="flex flex-wrap items-center gap-2">
                <label class="flex min-w-0 items-center gap-2">
                    checkbox(
                        attrs: attributes! {
                            aria-label="Select all of its files"
                            x-on:change="selectRows($el, $root)"
                        }
                    )
                    // A file's label is its path, which wraps anywhere.
                    <h2 class="min-w-0 font-semibold [overflow-wrap:anywhere]">
                        (mark.label.as_str())
                    </h2>
                </label>
                marking::progress_badge(progress: mark.progress)
                <div class="ml-auto flex gap-1">
                    <form method="post" action=(href!(exclude))>
                        <input type="hidden" name="mark" value=(mark.uuid.to_string())>
                        button(
                            variant: ButtonVariant::Ghost,
                            size: ButtonSize::Sm,
                            attrs: attributes! {
                                type="submit"
                                title="Leave its files out of the catalog, e.g. a sample, or a disc no knowledge base has"
                            },
                            "Exclude"
                        )
                    </form>
                    <form method="post" action=(href!(marking::remove_mark))>
                        <input type="hidden" name="mark" value=(mark.uuid.to_string())>
                        <input type="hidden" name="return_to" value=(return_to)>
                        button(
                            variant: ButtonVariant::Ghost,
                            size: ButtonSize::Sm,
                            attrs: attributes! { type="submit" },
                            "Remove mark"
                        )
                    </form>
                </div>
            </div>
            if !mark.note.is_empty() {
                <p class="text-sm">(mark.note.as_str())</p>
            }
            <ul class="flex flex-col text-sm">
                for row in &rows {
                    match row {
                        PathRow::Folder { depth, name, path, data_source_name } => {
                            <li
                                class="flex items-start gap-2 py-1"
                                style=(indent(*depth))
                                data-depth=(depth.to_string())
                            >
                                lucide(
                                    icon: FOLDER,
                                    attrs: attributes! { class="mt-0.5 size-4 shrink-0 text-muted-foreground" }
                                )
                                checkbox(
                                    attrs: attributes! {
                                        class="mt-0.5"
                                        name="folder"
                                        aria-label=(format!("Select the files in {path}/"))
                                    }
                                )
                                <div class="min-w-0 flex-1 [overflow-wrap:anywhere]">
                                    <span class="font-medium" translate="no">
                                        (format!("{name}/"))
                                    </span>
                                    if let Some(data_source_name) = data_source_name {
                                        <span
                                            class="ml-2 text-xs text-muted-foreground"
                                            translate="no"
                                        >
                                            (data_source_name.as_str())
                                        </span>
                                    }
                                </div>
                            </li>
                        }
                        PathRow::File { depth, index, show_data_source } => {
                            // The name and summary wrap as one block of text beside the
                            // checkbox, which stays by its first line.
                            <li
                                class="flex items-start gap-2 py-1"
                                style=(indent(*depth))
                                data-depth=(depth.to_string())
                            >
                                // Where a folder has its icon, so checkboxes line up within a
                                // level.
                                disc_icon(disc: files[*index].is_disc())
                                checkbox(
                                    attrs: attributes! {
                                        class="mt-0.5"
                                        form="identify"
                                        name="file"
                                        value=(FileKey::new(
                                            files[*index].file.data_source_uuid,
                                            &files[*index].file.rel_path.to_string_lossy(),
                                        ).to_value())
                                        aria-label=(format!(
                                            "Select {}",
                                            files[*index].file.rel_path.display(),
                                        ))
                                    }
                                )
                                <div class="min-w-0 flex-1 [overflow-wrap:anywhere]">
                                    <a
                                        class=(LINK)
                                        translate="no"
                                        href=(href!(
                                            rel_path::page,
                                            SourceId(files[*index].file.data_source_uuid.0),
                                            RelPath(rel_path::segments(&files[*index].file.rel_path)),
                                        ))
                                    >
                                        (files[*index]
                                            .listed
                                            .rsplit('/')
                                            .next()
                                            .unwrap_or_default())
                                    </a>
                                    if *show_data_source {
                                        <span
                                            class="ml-2 text-xs text-muted-foreground"
                                            translate="no"
                                        >
                                            (files[*index].file.data_source_name.as_str())
                                        </span>
                                    }
                                    <span class="ml-2 text-xs text-muted-foreground">
                                        file_summary(details: files[*index].details.as_ref())
                                    </span>
                                </div>
                            </li>
                        }
                    }
                }
            </ul>
        </section>
    })
}

/// Part of a file's summary: the words before a title, the title, which
/// translators leave be, and the words after it.
struct Phrase {
    before: String,
    title: String,
    after: String,
}

impl Phrase {
    fn words(text: String) -> Self {
        Self {
            before: text,
            title: String::new(),
            after: String::new(),
        }
    }
}

/// E.g. "1:35:41, main file of movie Perry Mason Returns (1985)". Titles are
/// kept apart from the words around them, so that translators leave them be.
#[component]
async fn file_summary(details: Option<&FileDetails>) -> Result<impl View> {
    let mut parts: Vec<Phrase> = Vec::new();
    if let Some(details) = details {
        // A disc says what it is, and its longest title's duration.
        let duration = match &details.disc {
            Some(disc) => Some(display::disc(disc.kind, disc.file_count, disc.duration)),
            None => details
                .media
                .as_ref()
                .and_then(|media| media.duration())
                .map(display::human_duration),
        };
        parts.extend(duration.map(Phrase::words));
        let placements = parts.len();
        parts.extend(details.movies.iter().map(|movie| {
            let role = display::movie_part(movie.role);
            let year = movie
                .release_year
                .map(|year| format!(" ({year})"))
                .unwrap_or_default();
            Phrase {
                before: format!("{role} of movie "),
                title: movie.title.clone(),
                after: year,
            }
        }));
        parts.extend(details.episodes.iter().map(|episode| {
            let number = display::episode_number(episode.season_number, episode.episode_number);
            Phrase {
                before: format!("{number} of series "),
                title: episode.series_name.clone(),
                after: String::new(),
            }
        }));
        parts.extend(details.series_extras.iter().map(|extra| {
            let part = if extra.disc { "disc" } else { "extra" };
            Phrase {
                before: format!("{part} of series "),
                title: extra.series_name.clone(),
                after: String::new(),
            }
        }));
        if parts.len() == placements {
            parts.push(Phrase::words("unidentified".to_string()));
        }
    } else {
        parts.push(Phrase::words("Not in the latest ingests".to_string()));
    }
    Ok(view! {
        for (i, Phrase { before, title, after }) in parts.iter().enumerate() {
            if i > 0 {
                ", "
            }
            (before.as_str())
            if !title.is_empty() {
                <span translate="no">(title.as_str())</span>
            }
            (after.as_str())
        }
    })
}

pub(super) async fn correct(cx: &Cx, files: &[PathId], claims: Vec<ClaimPredicate>) -> Result<()> {
    commands::correct_in_draft(app(cx), files, claims)
        .await
        .map_err(|e| PageError::log(e.into()))?;
    Ok(())
}

#[derive(Deserialize)]
struct ExcludeForm {
    mark: MarkUuid,
}

/// Excludes a mark's files from the catalog, in the draft, e.g. a sample or a
/// disc no knowledge base has, then shows the corrections again. Files that
/// follow them, e.g. a disc's streams, are excluded with them.
#[route(POST "./exclude")]
async fn exclude(cx: &Cx, Form(form): Form<ExcludeForm>) -> Result<SeeOther> {
    let mark = queries::list_marks(app(cx))
        .await?
        .into_iter()
        .find(|mark| mark.uuid == form.mark)
        .ok_or_not_found()?;
    let files: Vec<_> = mark
        .files
        .iter()
        .map(|file| PathId::new(file.data_source_uuid, &file.rel_path))
        .collect();
    correct(cx, &files, vec![ClaimPredicate::Excluded]).await?;
    Ok(see_other(href!(page).resolve(cx)))
}

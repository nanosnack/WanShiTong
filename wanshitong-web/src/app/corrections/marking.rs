//! Marking movies, series and files as needing correction, from the pages
//! where they're seen: the Mark button on a work's or file's page, the bar that
//! marks a selection, and the routes they post to. The marks are corrected on
//! the Corrections page, so the routes are under `/corrections`.

// A group: no URL segment of its own.
topcoat::router::segment!(kind = Group);

use crate::app::file_key::FileKey;
use crate::{
    app::{files::FolderSelection, tv_series::series_id::asserted_files},
    context::app,
    error::PageError,
    icons::{CHECK, lucide},
    ui::{
        badge::{BadgeVariant, badge},
        button::{ButtonSize, ButtonVariant, button},
        input::input,
    },
};
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{
        content::{Form, RawForm},
        error::{RouterErrorExt, SeeOther, bad_request, see_other},
        href, route,
    },
    view::{View, attributes, component, view},
};
use uuid::Uuid;
use wanshitong_app::domain::PathId;
use wanshitong_app::{
    commands::{self, NewMark},
    domain::{DataSourceUuid, FileRole, Mark, MarkProgress, MarkUuid, MovieUuid, TvSeriesUuid},
    queries,
};

#[component]
pub(super) async fn progress_badge(progress: MarkProgress) -> Result<impl View> {
    Ok(view! {
        match progress {
            MarkProgress::Open => {

            }
            MarkProgress::Pending => {
                badge(variant: BadgeVariant::Warning, "Pending")
            }
            MarkProgress::Done => {
                badge(variant: BadgeVariant::Success, "Done")
            }
        }
    })
}

/// The kind of thing a mark is made on, as named by the page the mark comes
/// from.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkKind {
    Movie,
    Series,
    File,
}

#[derive(Deserialize)]
struct MarkForm {
    kind: MarkKind,
    /// A movie's or series' UUID.
    uuid: Option<Uuid>,
    /// A file's data source and path.
    data_source: Option<Uuid>,
    path: Option<String>,
    #[serde(default)]
    note: String,
    return_to: String,
}

/// Marks a movie's, series' or file's files as needing correction, then shows
/// the page it was marked from again.
#[route(POST "./mark")]
async fn create_mark(cx: &Cx, Form(form): Form<MarkForm>) -> Result<SeeOther> {
    let MarkForm {
        kind,
        uuid,
        data_source,
        path,
        note,
        return_to,
    } = form;
    let (label, files) = match (kind, uuid, data_source, path) {
        (MarkKind::Movie | MarkKind::Series, Some(uuid), ..) => {
            work_to_mark(cx, kind, uuid).await?
        }
        (MarkKind::File, _, Some(data_source), Some(path)) => {
            let label = format!("File {path}");
            (label, vec![PathId::new(DataSourceUuid(data_source), path)])
        }
        _ => return Err(bad_request("nothing to mark").into()),
    };
    commands::mark_files(app(cx), NewMark { label, note, files })
        .await
        .map_err(|e| PageError::log(e.into()))?;
    Ok(see_other(local(&return_to)))
}

/// The mark-many form: what was selected, the note for all of it, and the
/// page to go back to.
#[derive(Default)]
struct MarkManyForm {
    works: Vec<(MarkKind, Uuid)>,
    files: Vec<(DataSourceUuid, String)>,
    folders: Vec<FolderSelection>,
    note: String,
    return_to: String,
    /// The work whose page they were selected on, if any, to name the marks
    /// by.
    in_work: Option<Uuid>,
}

impl MarkManyForm {
    /// The form as posted. Its fields repeat, which a typed form can't take.
    fn parse(body: &[u8]) -> Result<Self> {
        let mut form = Self::default();
        for (key, value) in form_urlencoded::parse(body) {
            match &*key {
                "work" => {
                    let work = match value.split_once(':') {
                        Some(("movie", uuid)) => {
                            uuid.parse().ok().map(|uuid| (MarkKind::Movie, uuid))
                        }
                        Some(("series", uuid)) => {
                            uuid.parse().ok().map(|uuid| (MarkKind::Series, uuid))
                        }
                        _ => None,
                    };
                    form.works.push(
                        work.ok_or_else(|| bad_request("a work is movie:UUID or series:UUID"))?,
                    );
                }
                "file" => {
                    let FileKey { data_source, path } = FileKey::from_value(&value)
                        .ok_or_else(|| bad_request("a file is DATA_SOURCE|PATH"))?;
                    form.files.push((data_source, path));
                }
                "folder" => form.folders.push(
                    FolderSelection::from_value(&value)
                        .ok_or_else(|| bad_request("not a folder"))?,
                ),
                "note" => form.note = value.into_owned(),
                "return_to" => form.return_to = value.into_owned(),
                "in_work" => {
                    form.in_work = Some(
                        value
                            .parse()
                            .map_err(|_| bad_request("not a work's UUID"))?,
                    )
                }
                _ => {}
            }
        }
        Ok(form)
    }
}

/// Marks each of the selected works and files, e.g. from the catalog's grid or
/// the files page, with the same note, then shows the page they were selected
/// on again. A work is `movie:UUID` or `series:UUID`, a file
/// a [`FileKey`], and a folder a [`FolderSelection`], for the files under
/// it that the files page listed. Each gets a mark of its own.
#[route(POST "./mark-many")]
pub(crate) async fn mark_many(cx: &Cx, RawForm(body): RawForm) -> Result<SeeOther> {
    let MarkManyForm {
        works,
        files,
        folders,
        note,
        return_to,
        in_work,
    } = MarkManyForm::parse(&body)?;
    // E.g. `Movie D2: The Mighty Ducks (1994): Folder D2/`.
    let prefix = match in_work {
        Some(uuid) => Some(work_title(cx, uuid).await?),
        None => None,
    };
    let labelled = |label: String| match &prefix {
        Some(prefix) => format!("{prefix}: {label}"),
        None => label,
    };
    let files = files.into_iter().map(|(data_source, path)| {
        let label = labelled(format!("File {path}"));
        (label, vec![PathId::new(data_source, path)])
    });
    let mut marks = Vec::new();
    for (kind, uuid) in works {
        marks.push(work_to_mark(cx, kind, uuid).await?);
    }
    for folder in folders {
        let files: Vec<_> = queries::list_files(app(cx), &folder.query())
            .await?
            .files
            .into_iter()
            .map(|file| PathId::new(file.data_source_uuid, file.rel_path))
            .collect();
        if !files.is_empty() {
            marks.push((labelled(folder.label()), files));
        }
    }
    for (label, files) in marks.into_iter().chain(files) {
        commands::mark_files(
            app(cx),
            NewMark {
                label,
                note: note.clone(),
                files,
            },
        )
        .await
        .map_err(|e| PageError::log(e.into()))?;
    }
    Ok(see_other(local(&return_to)))
}

/// What marking a movie or series records: its label, and its files. A
/// movie's disc files follow its index file, and a series' extras of other
/// files follow them, so neither is marked.
async fn work_to_mark(cx: &Cx, kind: MarkKind, uuid: Uuid) -> Result<(String, Vec<PathId>)> {
    match kind {
        MarkKind::Movie => {
            let movie = queries::get_movie(app(cx), MovieUuid(uuid))
                .await?
                .ok_or_not_found()?;
            let year = movie
                .release_year
                .map(|year| format!(" ({year})"))
                .unwrap_or_default();
            let files = movie
                .files
                .iter()
                .filter(|file| file.role != Some(FileRole::Disc))
                .map(|file| PathId::new(file.data_source_uuid, &file.rel_path))
                .collect();
            Ok((format!("Movie {}{year}", movie.title), files))
        }
        MarkKind::Series => {
            let series = queries::get_tv_series(app(cx), TvSeriesUuid(uuid))
                .await?
                .ok_or_not_found()?;
            let files = asserted_files(&series)
                .into_iter()
                .map(|file| PathId::new(file.data_source_uuid, &file.rel_path))
                .collect();
            Ok((format!("Series {}", series.name), files))
        }
        MarkKind::File => Err(bad_request("a file isn't a work").into()),
    }
}

/// A movie's or series' name, as marks name it, e.g. `Movie Heat (1995)` or
/// `Series The Wire`.
async fn work_title(cx: &Cx, uuid: Uuid) -> Result<String> {
    if let Some(movie) = queries::get_movie(app(cx), MovieUuid(uuid)).await? {
        let year = movie
            .release_year
            .map(|year| format!(" ({year})"))
            .unwrap_or_default();
        return Ok(format!("Movie {}{year}", movie.title));
    }
    let series = queries::get_tv_series(app(cx), TvSeriesUuid(uuid))
        .await?
        .ok_or_not_found()?;
    Ok(format!("Series {}", series.name))
}

#[derive(Deserialize)]
struct UnmarkForm {
    mark: MarkUuid,
    return_to: String,
}

/// Removes a mark, then shows the page it was removed from again.
#[route(POST "./unmark")]
pub(super) async fn remove_mark(cx: &Cx, Form(form): Form<UnmarkForm>) -> Result<SeeOther> {
    commands::unmark_files(app(cx), form.mark)
        .await
        .map_err(|e| PageError::log(e.into()))?;
    Ok(see_other(local(&form.return_to)))
}

/// A path on this site to go back to, so that a form can't send people
/// elsewhere.
fn local(path: &str) -> String {
    if path.starts_with('/') && !path.starts_with("//") {
        path.to_string()
    } else {
        "/".to_string()
    }
}

/// On a movie's, series' or file's page: a Mark button, with a field for a
/// note beside it, which turns to Marked once its files are marked as needing
/// correction, and removes the mark when pressed again.
#[component]
pub async fn mark_control(
    cx: &Cx,
    kind: MarkKind,
    /// A movie's or series' UUID, or a file's data source.
    uuid: Uuid,
    /// A file's path.
    #[default]
    path: Option<String>,
    files: Vec<PathId>,
    return_to: String,
) -> Result<impl View> {
    let marks = queries::list_marks(app(cx)).await?;
    let marked: Vec<Mark> = marks
        .into_iter()
        .filter(|mark| mark.progress != MarkProgress::Done)
        .filter(|mark| {
            mark.files
                .iter()
                .any(|file| files.contains(&PathId::new(file.data_source_uuid, &file.rel_path)))
        })
        .collect();
    let kind_value = match kind {
        MarkKind::Movie => "movie",
        MarkKind::Series => "series",
        MarkKind::File => "file",
    };
    Ok(view! {
        // Marking swaps only this, so the page stays as it was.
        <div id="mark-control" class="flex flex-wrap items-center gap-2" x-data="">
            match marked.first() {
                None => {
                    <form
                        class="flex flex-1 flex-wrap items-center gap-2"
                        method="post"
                        action=(href!(create_mark))
                        x-target.nofocus="mark-control"
                    >
                        <input type="hidden" name="kind" value=(kind_value)>
                        match (kind, &path) {
                            (MarkKind::File, Some(path)) => {
                                <input
                                    type="hidden"
                                    name="data_source"
                                    value=(uuid.to_string())
                                >
                                <input type="hidden" name="path" value=(path.as_str())>
                            }
                            _ => {
                                <input type="hidden" name="uuid" value=(uuid.to_string())>
                            }
                        }
                        <input
                            type="hidden"
                            name="return_to"
                            value=(return_to.as_str())
                        >
                        input(
                            attrs: attributes! {
                                name="note"
                                aria-label="Note"
                                placeholder="Note, if any, e.g. this is a TV series"
                                class="min-w-48 flex-1"
                            }
                        )
                        button(
                            variant: ButtonVariant::Outline,
                            size: ButtonSize::Sm,
                            attrs: attributes! {
                                type="submit"
                                title="Mark as needing correction, to correct on the Corrections page"
                            },
                            "Mark"
                        )
                    </form>
                }
                Some(mark) => {
                    if !mark.note.is_empty() {
                        <span class="flex-1 text-sm text-muted-foreground">
                            (mark.note.as_str())
                        </span>
                    }
                    progress_badge(progress: mark.progress)
                    <a class="text-sm underline" href=(href!(super::page))>
                        "Corrections"
                    </a>
                    <form
                        class="ml-auto"
                        method="post"
                        action=(href!(remove_mark))
                        x-target.nofocus="mark-control"
                    >
                        <input type="hidden" name="mark" value=(mark.uuid.to_string())>
                        <input
                            type="hidden"
                            name="return_to"
                            value=(return_to.as_str())
                        >
                        button(
                            variant: ButtonVariant::Secondary,
                            size: ButtonSize::Sm,
                            attrs: attributes! {
                                type="submit"
                                title="Marked as needing correction. Click to remove the mark."
                            },
                            lucide(icon: CHECK, attrs: attributes! { class="size-4" })
                            "Marked"
                        )
                    </form>
                }
            }
        </div>
    })
}

/// Floats at the bottom of a page while any of its works or files are
/// selected, to mark them all as needing correction, with one note. Put it
/// in an element whose Alpine component keeps `selected`, the count of its
/// selections, e.g. `selection` (ts/selection.ts) or `fileTree` (ts/tree.ts),
/// around checkboxes with `form="mark-selected"`, named `work` (`movie:UUID`
/// or `series:UUID`), `file` (`DATA_SOURCE|PATH`) or `folder`; see
/// [`mark_many`].
/// The bar that floats up from the bottom of a page while anything is
/// selected, holding what to do with the selection.
pub const SELECTED_BAR: &str = concat!(
    "fixed inset-x-4 bottom-4 z-20 mx-auto flex max-w-xl flex-wrap items-center gap-2 ",
    "rounded-lg border border-border bg-background p-2 shadow-lg",
);

#[component]
pub async fn mark_selected_bar(
    /// The page, to come back to after marking.
    return_to: String,
    /// The work whose page they're selected on, if any, which names the
    /// marks, e.g. `Movie Heat (1995): Folder Heat/`.
    #[default]
    in_work: Option<Uuid>,
) -> Result<impl View> {
    Ok(view! {
        <form
            id="mark-selected"
            method="post"
            action=(href!(mark_many))
            x-show="selected > 0"
            style="display: none"
            class=(SELECTED_BAR)
        >
            <span class="px-2 text-sm font-medium" x-text="`${selected} selected`"></span>
            <input type="hidden" name="return_to" value=(return_to.as_str())>
            if let Some(work) = in_work {
                <input type="hidden" name="in_work" value=(work.to_string())>
            }
            input(
                attrs: attributes! {
                    name="note"
                    aria-label="Note"
                    placeholder="Note, if any"
                    class="min-w-32 flex-1"
                }
            )
            button(
                size: ButtonSize::Sm,
                attrs: attributes! {
                    type="submit"
                    title="Mark as needing correction, to correct on the Corrections page"
                },
                "Mark"
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

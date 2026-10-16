//! Files, each at `/files/<data source uuid>/<path>`.

pub mod source_id;

use crate::app::file_key::FileKey;
use crate::app::page_help::page_help;
use crate::app::path_tree::{PathRow, ROW, ROW_TEXT, indent, listed_path, path_rows};
use crate::{
    app::corrections,
    context::app,
    cursor::{decode_cursor, encode_cursor},
    display::{self, FileSize},
    icons::{CHEVRON_RIGHT, DISC, FOLDER, lucide},
    styles::{LINK, NUMBER, SECTION_H1, SECTION_HEADING_ROW},
    ui::{checkbox::checkbox, input::input},
};
use serde::{Deserialize, Serialize};
use source_id::{
    SourceId,
    rel_path::{self, RelPath},
};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::sync::Arc;
use topcoat::{
    Result,
    context::Cx,
    router::{href, page, query_params},
    view::{View, ViewExt, attributes, class, component, view},
};
use uuid::Uuid;
use wanshitong_app::domain::{DataSourceUuid, MovieUuid, TvSeriesUuid};
use wanshitong_app::queries::{
    self, FileSearchQuery, FileSummaryList, FileTreeEntry, FileTreeList, FileTreeQuery,
};

#[query_params(error = bad_request)]
struct Search {
    name: Option<String>,
    /// Also the files of identified works. The checkboxes send "true" when
    /// checked and nothing otherwise.
    identified: Option<bool>,
    /// Also the files without moving pictures, e.g. subtitles or scans.
    non_videos: Option<bool>,
    /// Where the next page of a search's files starts.
    cursor: Option<String>,
    /// The last file a search's previous page showed, whose folders already
    /// have their rows.
    previous: Option<String>,
    /// A folder to list the children of, as `DATA_SOURCE|PATH`, rather than
    /// the whole page, e.g. when it's expanded in the tree.
    folder: Option<String>,
    /// Where the next page of a folder's children starts.
    after: Option<String>,
    /// Only a movie's or series' files, all of them, e.g. on its page.
    work: Option<Uuid>,
}

/// Which files are listed: the unidentified videos, unless checkboxes add
/// identified files or files that aren't videos.
#[derive(Clone, Copy)]
struct Filter {
    identified: bool,
    non_videos: bool,
    /// A movie's or series' UUID, for all of its files instead.
    work: Option<Uuid>,
}

impl Filter {
    fn params(self) -> Vec<(&'static str, String)> {
        let mut params = Vec::new();
        if self.identified {
            params.push(("identified", "true".to_string()));
        }
        if self.non_videos {
            params.push(("non_videos", "true".to_string()));
        }
        if let Some(work) = self.work {
            params.push(("work", work.to_string()));
        }
        params
    }
}

/// A selected folder, as its checkbox's value: marking it marks the files
/// under it that the page lists, with its filter and search, including those
/// not loaded yet.
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct FolderSelection {
    /// None for a search's folder, whose results can be in several data
    /// sources.
    pub data_source: Option<DataSourceUuid>,
    pub path: String,
    /// The search, if any.
    pub name: Option<String>,
    pub identified: bool,
    pub non_videos: bool,
    /// A work's page's folder: its files under it.
    #[serde(default)]
    pub work: Option<Uuid>,
}

impl FolderSelection {
    fn to_value(&self) -> String {
        serde_json::to_string(self).expect("a folder serializes")
    }

    /// A folder from its checkbox's value.
    pub(crate) fn from_value(value: &str) -> Option<Self> {
        serde_json::from_str(value).ok()
    }

    /// The files it selects.
    pub(crate) fn query(&self) -> FileSearchQuery {
        FileSearchQuery {
            name: self.name.clone(),
            folder: Some((self.data_source, self.path.clone())),
            work: self.work,
            identified: self.identified,
            non_videos: self.non_videos,
            limit: i64::MAX - 1,
            ..Default::default()
        }
    }

    /// What its mark is called, e.g. `Folder Show/`.
    pub(crate) fn label(&self) -> String {
        match &self.name {
            Some(name) => format!("Folder {}/, files matching “{name}”", self.path),
            None => format!("Folder {}/", self.path),
        }
    }
}

/// How many files, or a folder's children, to list at a time.
const PAGE: i64 = 200;

/// What the page lists: the files matching a name search, or the tree of
/// folders.
enum Listing {
    Search(FileSummaryList),
    Tree(FileTreeList),
}

/// The files as a tree of folders, all collapsed at first, each expanded on
/// request, or the files matching a name search, as a list. Either way, the
/// files are the unidentified videos, which are what's left to identify, unless
/// checkboxes add identified files or files that aren't videos.
#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let Search {
        name,
        identified,
        non_videos,
        cursor,
        previous,
        folder,
        after,
        work,
    } = query_params::<Search>(cx)?;
    let previous: Option<(String, String)> = previous.as_deref().and_then(decode_cursor);
    let filter = Filter {
        identified: *identified == Some(true),
        non_videos: *non_videos == Some(true),
        work: *work,
    };
    let name = name.clone().filter(|name| !name.trim().is_empty());
    let folder = folder.as_deref().and_then(FileKey::from_value);
    let after = after.as_deref().and_then(decode_cursor);

    // A folder's children, for its tree to merge in.
    if let Some(folder) = folder {
        let list = tree_list(cx, Some(folder.clone()), filter, after).await?;
        let labels = work_labels(cx, filter.work).await?;
        return Ok(view! {
            tree_level(
                id: folder_id(&folder),
                folder: Some(folder),
                list: list,
                filter: filter,
                labels: labels
            )
        }
        .boxed());
    }

    let mut params = filter.params();
    if let Some(name) = &name {
        params.insert(0, ("name", name.clone()));
    }
    let here = href!(page).query(params.clone()).resolve(cx);
    let files = match &name {
        Some(name) => {
            let list = queries::list_files(
                app(cx),
                &FileSearchQuery {
                    name: Some(name.clone()),
                    folder: None,
                    work: filter.work,
                    identified: filter.identified,
                    non_videos: filter.non_videos,
                    cursor: cursor.as_deref().and_then(decode_cursor),
                    limit: PAGE,
                },
            )
            .await?;
            Listing::Search(list)
        }
        None => Listing::Tree(tree_list(cx, None, filter, after).await?),
    };
    let labels = work_labels(cx, filter.work).await?;

    Ok(view! {
        <div class=(SECTION_HEADING_ROW)>
            <h1 class=(SECTION_H1)>"Files"</h1>
            page_help(
                section: true,
                <p>
                    "The files seen by the canonical interpretation. "
                    "Open one to see what it is in the catalog and how it was identified."
                </p>
                <p>
                    "By default, this queries only unidentified video files. "
                    "“Show identified files” adds the files of identified movies and series. "
                    "“Show non-videos” adds files without video streams. "
                    "With both, it lists every file."
                </p>
            )
        </div>

        <form
            class="flex flex-wrap items-center gap-x-4 gap-y-2"
            method="get"
            action=(href!(page))
            x-data=""
            x-target.push.nofocus="file-list"
            x-on:input.debounce.300ms="$el.requestSubmit()"
        >
            input(
                attrs: attributes! {
                    type="search"
                    name="name"
                    placeholder="Search files…"
                    value=(name.as_deref().unwrap_or_default())
                    class="min-w-48 flex-1"
                }
            )
            <label
                class="flex items-center gap-2 text-sm"
                title="Also the files of identified movies and series"
            >
                checkbox(
                    attrs: attributes! {
                        name="identified"
                        value="true"
                        checked=(filter.identified)
                    }
                )
                "Show identified files"
            </label>
            <label
                class="flex items-center gap-2 text-sm"
                title="Also files without moving pictures, e.g. subtitles or scans"
            >
                checkbox(
                    attrs: attributes! {
                        name="non_videos"
                        value="true"
                        checked=(filter.non_videos)
                    }
                )
                "Show non-videos"
            </label>
        </form>

        // Selecting files and folders marks them, with the bar this counts them for.
        // See ts/tree.ts.
        <div
            id="file-list"
            x-data="fileTree"
            x-on:change="changed($event)"
            x-on:submit="submitting()"
        >
            match files {
                Listing::Search(list) => {
                    search_results(
                        list: list,
                        params: params,
                        previous: previous,
                        search: name.clone().unwrap_or_default(),
                        filter: filter
                    )
                }
                Listing::Tree(list) => {
                    if list.entries.is_empty() {
                        <p class="text-sm text-muted-foreground">"No matching files"</p>
                    }
                    tree_level(
                        id: "file-tree".to_string(),
                        folder: None,
                        list: list,
                        filter: filter,
                        labels: labels
                    )
                }
            }
            corrections::marking::mark_selected_bar(return_to: here)
        </div>
    }
    .boxed())
}

async fn tree_list(
    cx: &Cx,
    folder: Option<FileKey>,
    filter: Filter,
    after: Option<queries::FileTreeCursor>,
) -> Result<FileTreeList> {
    Ok(queries::list_file_tree(
        app(cx),
        &FileTreeQuery {
            folder: folder.map(|folder| (folder.data_source, folder.path)),
            work: filter.work,
            identified: filter.identified,
            non_videos: filter.non_videos,
            after,
            limit: PAGE,
        },
    )
    .await?)
}

/// The id of the list of a folder's children, which its expansion and its
/// pages merge into: the same for the same folder in every response.
fn folder_id(folder: &FileKey) -> String {
    let mut hasher = DefaultHasher::new();
    (folder.data_source.0, &folder.path).hash(&mut hasher);
    format!("folder-{:016x}", hasher.finish())
}

/// A folder's children, or the top level's, by name, then a row that loads
/// the next page once it's scrolled to.
#[component]
async fn tree_level(
    id: String,
    folder: Option<FileKey>,
    list: FileTreeList,
    filter: Filter,
    labels: Labels,
) -> Result<impl View> {
    let top = folder.is_none();
    let next = list.next.as_ref().map(|after| {
        let mut params = filter.params();
        if let Some(folder) = &folder {
            params.push(("folder", folder.to_value()));
        }
        params.push(("after", encode_cursor(after)));
        href!(page).query(params)
    });
    let class = if top {
        "flex flex-col text-sm"
    } else {
        "ml-2 flex flex-col border-l border-border pl-3"
    };
    Ok(view! {
        <ul id=(id.as_str()) class=(class) x-merge="append">
            for entry in &list.entries {
                if entry.is_folder {
                    folder_entry(entry: entry, top: top, filter: filter)
                } else {
                    file_entry(
                        entry: entry,
                        top: top,
                        label: labels
                            .get(&(entry.data_source_uuid, entry.path.clone()))
                            .cloned()
                    )
                }
            }
            if let Some(next) = next {
                <li
                    class="py-1 text-muted-foreground"
                    data-next=(next)
                    data-target=(id.as_str())
                    x-intersect.once="more($el)"
                >
                    "Loading more…"
                </li>
            }
        </ul>
    })
}

/// A folder, collapsed, whose children load the first time it's expanded.
#[component]
async fn folder_entry(entry: &FileTreeEntry, top: bool, filter: Filter) -> Result<impl View> {
    let folder = FileKey::new(entry.data_source_uuid, &entry.path);
    let mut params = filter.params();
    params.push(("folder", folder.to_value()));
    let children = href!(page).query(params);
    let selection = FolderSelection {
        data_source: Some(entry.data_source_uuid),
        path: entry.path.clone(),
        name: None,
        identified: filter.identified,
        non_videos: filter.non_videos,
        work: filter.work,
    }
    .to_value();
    let count = match (
        filter.identified || filter.non_videos || filter.work.is_some(),
        entry.file_count,
    ) {
        (false, 1) => "1 unidentified video".to_string(),
        (false, n) => format!("{n} unidentified videos"),
        (true, 1) => "1 file".to_string(),
        (true, n) => format!("{n} files"),
    };
    Ok(view! {
        <li>
            <details
                data-children=(children)
                data-target=(folder_id(&folder))
                x-on:toggle="if ($el.open) load($el)"
            >
                <summary
                    class=(class!(
                        ROW,
                        "cursor-pointer list-none rounded-md hover:bg-foreground/5",
                        "[&::-webkit-details-marker]:hidden",
                    ))
                >
                    // Turned by its own folder being open, not by any folder around it.
                    lucide(
                        icon: CHEVRON_RIGHT,
                        attrs: attributes! {
                            class=(class!(
                                "size-4 shrink-0 text-muted-foreground transition-transform",
                                "[details[open]>summary>&]:rotate-90",
                            ))
                        }
                    )
                    checkbox(
                        attrs: attributes! {
                            form="mark-selected"
                            name="folder"
                            value=(selection)
                            aria-label=(format!("Select {}/", entry.path))
                        }
                    )
                    // In the page's font: a monospace font sets each glyph in a cell as wide
                    // as any, so names that start with a narrow one, e.g. `[`, look indented.
                    <div class=(ROW_TEXT)>
                        <span
                            class="min-w-0 font-medium [overflow-wrap:anywhere]"
                            translate="no"
                        >
                            (format!("{}/", entry.name))
                        </span>
                        if top {
                            <span class="text-xs text-muted-foreground" translate="no">
                                (entry.data_source_name.as_str())
                            </span>
                        }
                        <span
                            class="ml-auto shrink-0 text-xs whitespace-nowrap text-muted-foreground"
                        >
                            (count)
                        </span>
                    </div>
                </summary>
                <ul
                    id=(folder_id(&folder))
                    x-merge="append"
                    class="ml-2 flex flex-col border-l border-border pl-3"
                ></ul>
            </details>
        </li>
    })
}

/// A file, with a checkbox to select it for marking.
#[component]
async fn file_entry(
    entry: &FileTreeEntry,
    top: bool,
    /// What it is of the work whose files the tree shows, e.g. `S01E02`.
    label: Option<String>,
) -> Result<impl View> {
    Ok(view! {
        <li class=(ROW)>
            // Where a folder has its chevron, so checkboxes line up within a level:
            // a disc's icon, as a disc is one item, not a folder to open.
            disc_icon(disc: entry.disc.is_some())
            checkbox(
                attrs: attributes! {
                    form="mark-selected"
                    name="file"
                    value=(FileKey::new(entry.data_source_uuid, &entry.path).to_value())
                    aria-label=(format!("Select {}", entry.path))
                }
            )
            <div class=(ROW_TEXT)>
                <a
                    class=(format!("{LINK} min-w-0 [overflow-wrap:anywhere]"))
                    translate="no"
                    href=(href!(
                        rel_path::page,
                        SourceId(entry.data_source_uuid.0),
                        RelPath(rel_path::segments(Path::new(&entry.path))),
                    ))
                >
                    (entry.name.as_str())
                </a>
                if top {
                    <span class="text-xs text-muted-foreground" translate="no">
                        (entry.data_source_name.as_str())
                    </span>
                }
                if let Some(disc) = entry.disc {
                    <span class="text-xs text-muted-foreground">
                        (display::disc(disc.kind, disc.file_count, disc.duration))
                    </span>
                }
                if let Some(label) = label {
                    <span class="text-xs text-muted-foreground">(label)</span>
                }
                <span
                    class=(format!(
                        "{NUMBER} ml-auto shrink-0 text-xs whitespace-nowrap text-muted-foreground",
                    ))
                >
                    (FileSize(Some(entry.size)).to_string())
                </span>
            </div>
        </li>
    })
}

/// What each file is of the work whose files a tree shows, by data source and
/// path, e.g. `main file` or `S01E02`.
type Labels = Arc<HashMap<(DataSourceUuid, String), String>>;

/// The label for each of a work's files, e.g. `main file` or `S01E02`, when
/// the tree shows one work's files.
async fn work_labels(cx: &Cx, work: Option<Uuid>) -> Result<Labels> {
    let mut labels = HashMap::new();
    let key = |data_source: DataSourceUuid, path: &Path| (data_source, path.display().to_string());
    if let Some(work) = work {
        if let Some(movie) = queries::get_movie(app(cx), MovieUuid(work)).await? {
            for file in movie.files {
                let label = display::movie_part(file.role);
                labels.insert(
                    key(file.data_source_uuid, &file.rel_path),
                    label.to_string(),
                );
            }
        } else if let Some(series) = queries::get_tv_series(app(cx), TvSeriesUuid(work)).await? {
            for season in &series.seasons {
                for episode in &season.episodes {
                    let label = display::episode_number(season.number, episode.number);
                    for file in &episode.files {
                        // A file of several episodes is each of them.
                        labels
                            .entry(key(file.data_source_uuid, &file.rel_path))
                            .and_modify(|labels: &mut String| {
                                *labels = format!("{labels}, {label}")
                            })
                            .or_insert_with(|| label.clone());
                    }
                }
            }
            for file in &series.extras {
                labels.insert(
                    key(file.data_source_uuid, &file.rel_path),
                    "extra".to_string(),
                );
            }
            for file in &series.discs {
                labels.insert(
                    key(file.data_source_uuid, &file.rel_path),
                    "disc".to_string(),
                );
            }
        }
    }
    Ok(Arc::new(labels))
}

/// A movie's or series' files, as a tree of the folders they're in, with
/// what each is of it, to select and mark them, as on the files page. The tree
/// opens down to the first folder with more than one thing in it.
#[component]
pub async fn work_files(
    cx: &Cx,
    /// The movie's or series' UUID.
    work: Uuid,
    /// The page, to come back to after marking.
    return_to: String,
) -> Result<impl View> {
    let filter = Filter {
        identified: false,
        non_videos: false,
        work: Some(work),
    };
    let list = tree_list(cx, None, filter, None).await?;
    let labels = work_labels(cx, filter.work).await?;
    Ok(view! {
        // See ts/tree.ts.
        <div
            x-data="fileTree"
            x-init="openSingle($el.querySelector('#file-tree'))"
            x-on:change="changed($event)"
            x-on:submit="submitting()"
        >
            <label class="mb-1 flex items-center gap-2 text-sm text-muted-foreground">
                checkbox(
                    attrs: attributes! {
                        aria-label="Select all of its files"
                        x-on:change="selectAll($event)"
                    }
                )
                "Select all"
            </label>
            tree_level(
                id: "file-tree".to_string(),
                folder: None,
                list: list,
                filter: filter,
                labels: labels
            )
            corrections::marking::mark_selected_bar(
                return_to: return_to,
                in_work: Some(work)
            )
        </div>
    })
}

/// The files matching a name search, by folder: each run of results in a
/// folder under a row for it. The next page loads once it's scrolled to.
#[component]
async fn search_results(
    list: FileSummaryList,
    params: Vec<(&'static str, String)>,
    previous: Option<(String, String)>,
    search: String,
    filter: Filter,
) -> Result<impl View> {
    let FileSummaryList { files, next_cursor } = list;
    // Marking a folder marks the results under it, in whichever data sources.
    let selection = move |path: &str| {
        FolderSelection {
            data_source: None,
            path: path.to_string(),
            name: Some(search.clone()),
            identified: filter.identified,
            non_videos: filter.non_videos,
            work: filter.work,
        }
        .to_value()
    };
    let next_page = next_cursor.map(|cursor| {
        let mut params = params.clone();
        params.push(("cursor", encode_cursor(&cursor)));
        if let Some(last) = files.last() {
            params.push((
                "previous",
                encode_cursor(&(
                    &last.data_source_name,
                    listed_path(&last.rel_path, last.disc.is_some()),
                )),
            ));
        }
        href!(page).query(params)
    });
    let first_page = previous.is_none();
    let rows = {
        let paths: Vec<(&str, &str)> = files
            .iter()
            .map(|file| {
                let path = listed_path(&file.rel_path, file.disc.is_some());
                (file.data_source_name.as_str(), path)
            })
            .collect();
        let previous = previous
            .as_ref()
            .map(|(data_source_name, path)| (data_source_name.as_str(), path.as_str()));
        path_rows(&paths, previous)
    };
    Ok(view! {
        if files.is_empty() && first_page {
            <p class="text-sm text-muted-foreground">"No matching files"</p>
        }
        <ul id="file-results" class="flex flex-col text-sm" x-merge="append">
            for row in &rows {
                match row {
                    PathRow::Folder { depth, name, path, data_source_name } => {
                        <li
                            class=(ROW)
                            style=(indent(*depth))
                            data-depth=(depth.to_string())
                        >
                            lucide(
                                icon: FOLDER,
                                attrs: attributes! { class="size-4 shrink-0 text-muted-foreground" }
                            )
                            checkbox(
                                attrs: attributes! {
                                    form="mark-selected"
                                    name="folder"
                                    value=(selection(path))
                                    aria-label=(format!("Select {path}/"))
                                }
                            )
                            <div class=(ROW_TEXT)>
                                <span
                                    class="min-w-0 font-medium [overflow-wrap:anywhere]"
                                    translate="no"
                                >
                                    (format!("{name}/"))
                                </span>
                                if let Some(data_source_name) = data_source_name {
                                    <span class="text-xs text-muted-foreground" translate="no">
                                        (data_source_name.as_str())
                                    </span>
                                }
                            </div>
                        </li>
                    }
                    PathRow::File { depth, index, show_data_source } => {
                        <li
                            class=(ROW)
                            style=(indent(*depth))
                            data-depth=(depth.to_string())
                        >
                            // Where a folder has its icon, so checkboxes line up within a level.
                            disc_icon(disc: files[*index].disc.is_some())
                            checkbox(
                                attrs: attributes! {
                                    form="mark-selected"
                                    name="file"
                                    value=(FileKey::new(
                                        files[*index].data_source_uuid,
                                        &files[*index].rel_path,
                                    ).to_value())
                                    aria-label=(format!("Select {}", files[*index].rel_path))
                                }
                            )
                            <div class=(ROW_TEXT)>
                                <a
                                    class=(format!("{LINK} min-w-0 [overflow-wrap:anywhere]"))
                                    translate="no"
                                    href=(href!(
                                        rel_path::page,
                                        SourceId(files[*index].data_source_uuid.0),
                                        RelPath(
                                            rel_path::segments(Path::new(&files[*index].rel_path)),
                                        ),
                                    ))
                                >
                                    (listed_path(
                                        &files[*index].rel_path,
                                        files[*index].disc.is_some(),
                                    ).rsplit('/').next().unwrap_or_default())
                                </a>
                                if let Some(disc) = files[*index].disc {
                                    <span class="text-xs text-muted-foreground">
                                        (display::disc(disc.kind, disc.file_count, disc.duration))
                                    </span>
                                }
                                if *show_data_source {
                                    <span class="text-xs text-muted-foreground" translate="no">
                                        (files[*index].data_source_name.as_str())
                                    </span>
                                }
                                <span
                                    class=(class!(
                                        NUMBER,
                                        "ml-auto shrink-0 text-xs whitespace-nowrap",
                                        "text-muted-foreground",
                                    ))
                                >
                                    (FileSize(files[*index].size).to_string())
                                </span>
                            </div>
                        </li>
                    }
                }
            }
            if let Some(next_page) = next_page {
                <li
                    class="py-1 text-muted-foreground"
                    data-next=(next_page)
                    data-target="file-results"
                    x-intersect.once="more($el)"
                >
                    "Loading more…"
                </li>
            }
        </ul>
    })
}

/// What goes before a file's checkbox, where a folder has its chevron: a
/// disc's icon, or nothing.
#[component]
pub(crate) async fn disc_icon(disc: bool) -> Result<impl View> {
    Ok(view! {
        if disc {
            lucide(
                icon: DISC,
                attrs: attributes! { class="size-4 shrink-0 text-muted-foreground" }
            )
        } else {
            <span class="size-4 shrink-0" aria-hidden="true"></span>
        }
    })
}

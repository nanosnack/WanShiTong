//! Lists of files as trees of the folders they're in: each run of files in a
//! folder under a row for it, indented by depth, so that a list of long paths
//! reads as a hierarchy. Pages render the rows with what they show of each
//! file, e.g. the files page's search results and the Corrections page's marks.

use crate::{
    icons::{FOLDER, lucide},
    styles::NUMBER,
};
use std::cmp::Ordering;
use topcoat::{
    Result,
    view::{View, attributes, component, view},
};
use wanshitong_app::domain::disc_folder;

/// A row of a tree of files: a folder, where a run of files in it starts, or a
/// file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathRow {
    Folder {
        depth: usize,
        name: String,
        /// Its path, in whichever data sources its files are.
        path: String,
        /// For a top-level folder: the data source of the file that starts it.
        data_source_name: Option<String>,
    },
    File {
        depth: usize,
        /// Which of the files it is.
        index: usize,
        /// Whether to name the file's data source: true when the file is at the
        /// top level, or when its data source differs from the one its top-level
        /// folder's row names.
        show_data_source: bool,
    },
}

/// How files are ordered in a tree: by path, ignoring case, whatever data
/// source they're in, as the files queries order them.
pub fn path_order(a: (&str, &str), b: (&str, &str)) -> Ordering {
    let ((a_source, a_path), (b_source, b_path)) = (a, b);
    (a_path.to_lowercase(), a_path, a_source).cmp(&(b_path.to_lowercase(), b_path, b_source))
}

/// The rows for `files`, each its data source's name and its path, in
/// [`path_order`], after `previous`, the last file shown before them, whose
/// folders already have their rows. Folders are grouped by path, whichever data
/// sources their files are in.
pub fn path_rows(files: &[(&str, &str)], previous: Option<(&str, &str)>) -> Vec<PathRow> {
    fn folders(path: &str) -> Vec<&str> {
        let mut segments: Vec<&str> = path.split('/').collect();
        segments.pop();
        segments
    }
    let (mut open, mut top_data_source) = match previous {
        Some((data_source_name, path)) => (folders(path), Some(data_source_name)),
        None => (Vec::new(), None),
    };
    let mut rows = Vec::new();
    for (index, &(data_source_name, path)) in files.iter().enumerate() {
        let folders = folders(path);
        // The folders it shares with the file before it.
        let shared = open
            .iter()
            .zip(&folders)
            .take_while(|(a, b)| a == b)
            .count();
        for (depth, name) in folders.iter().enumerate().skip(shared) {
            if depth == 0 {
                top_data_source = Some(data_source_name);
            }
            rows.push(PathRow::Folder {
                depth,
                name: name.to_string(),
                path: folders[..=depth].join("/"),
                data_source_name: (depth == 0).then(|| data_source_name.to_string()),
            });
        }
        rows.push(PathRow::File {
            depth: folders.len(),
            index,
            show_data_source: folders.is_empty() || top_data_source != Some(data_source_name),
        });
        open = folders;
    }
    rows
}

/// The path a file is listed at: a disc's folder's, e.g. `Film/BDMV`, where
/// its files are, rather than its index file's, which stands for them.
pub fn listed_path(rel_path: &str, is_disc: bool) -> &str {
    match disc_folder(rel_path) {
        folder if is_disc && !folder.is_empty() => folder,
        _ => rel_path,
    }
}

/// Indents a row by its depth, as the files page's tree does its levels.
pub fn indent(depth: usize) -> String {
    format!("padding-left: {}rem", depth as f32 * 1.25)
}

/// A row: its icons, then its [`ROW_TEXT`]. All rows are the same height,
/// whatever fonts their parts are in, and the icons stay by the text's first
/// line when it wraps.
pub const ROW: &str = "flex min-h-7 items-start gap-2 py-1 [&>:not(:last-child)]:mt-0.5";

/// A row's name, then what's said of it, e.g. its size. On a narrow screen a
/// long name has a line of its own, and what's said of it wraps beneath it.
pub const ROW_TEXT: &str = "flex min-w-0 flex-1 flex-wrap items-center gap-x-2";

/// A file in a [`path_list`]: its data source, its path, and a note, if any,
/// e.g. that its assertion was retracted.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct ListedFile {
    pub data_source_name: String,
    pub path: String,
    pub note: Option<String>,
}

/// Files as a tree of the folders they're in, to read, e.g. an assertion's.
#[component]
pub async fn path_list(mut files: Vec<ListedFile>) -> Result<impl View> {
    files.sort_by(|a, b| {
        path_order(
            (&a.data_source_name, &a.path),
            (&b.data_source_name, &b.path),
        )
    });
    let paths: Vec<(&str, &str)> = files
        .iter()
        .map(|file| (file.data_source_name.as_str(), file.path.as_str()))
        .collect();
    let rows = path_rows(&paths, None);
    Ok(view! {
        <ul class="flex flex-col text-sm">
            for row in &rows {
                match row {
                    PathRow::Folder { depth, name, data_source_name, .. } => {
                        <li class=(ROW) style=(indent(*depth))>
                            lucide(
                                icon: FOLDER,
                                attrs: attributes! { class="size-4 shrink-0 text-muted-foreground" }
                            )
                            <div class=(ROW_TEXT)>
                                <span
                                    class="min-w-0 font-medium [overflow-wrap:anywhere]"
                                    translate="no"
                                >
                                    (format!("{name}/"))
                                </span>
                                if let Some(data_source_name) = data_source_name {
                                    <span
                                        class="shrink-0 text-xs text-muted-foreground"
                                        translate="no"
                                    >
                                        (data_source_name.as_str())
                                    </span>
                                }
                            </div>
                        </li>
                    }
                    PathRow::File { depth, index, show_data_source } => {
                        <li class=(ROW) style=(indent(*depth))>
                            // Where a folder has its icon, so names line up within a level.
                            <span class="size-4 shrink-0" aria-hidden="true"></span>
                            <div class=(ROW_TEXT)>
                                <span
                                    class="min-w-0 [overflow-wrap:anywhere]"
                                    translate="no"
                                >
                                    (files[*index].path.rsplit('/').next().unwrap_or_default())
                                </span>
                                if *show_data_source {
                                    <span
                                        class="shrink-0 text-xs text-muted-foreground"
                                        translate="no"
                                    >
                                        (files[*index].data_source_name.as_str())
                                    </span>
                                }
                                if let Some(note) = &files[*index].note {
                                    <span
                                        class=(format!("{NUMBER} text-xs text-muted-foreground"))
                                    >
                                        (note.as_str())
                                    </span>
                                }
                            </div>
                        </li>
                    }
                }
            }
        </ul>
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each row as its depth and a folder's name with a slash, or a file's path.
    fn summary(files: &[(&str, &str)], rows: &[PathRow]) -> Vec<(usize, String)> {
        rows.iter()
            .map(|row| match row {
                PathRow::Folder { depth, name, .. } => (*depth, format!("{name}/")),
                PathRow::File { depth, index, .. } => (*depth, files[*index].1.to_string()),
            })
            .collect()
    }

    #[test]
    fn files_are_grouped_under_the_folders_they_share() {
        // By path, whatever data source they're in.
        let files = [
            ("lib", "Show/S01/e01.mkv"),
            ("other", "Show/S01/e01.mkv"),
            ("lib", "Show/S01/e02.mkv"),
            ("lib", "Show/S02/e01.mkv"),
            ("lib", "top.mkv"),
        ];
        let rows = path_rows(&files, None);
        let row = |depth, text: &str| (depth, text.to_string());
        assert_eq!(
            summary(&files, &rows),
            [
                row(0, "Show/"),
                row(1, "S01/"),
                row(2, "Show/S01/e01.mkv"),
                row(2, "Show/S01/e01.mkv"),
                row(2, "Show/S01/e02.mkv"),
                row(1, "S02/"),
                row(2, "Show/S02/e01.mkv"),
                row(0, "top.mkv"),
            ]
        );
        // Files say which data source they're in when their folder's row doesn't.
        let shown: Vec<bool> = rows
            .iter()
            .filter_map(|row| match row {
                PathRow::File {
                    show_data_source, ..
                } => Some(*show_data_source),
                PathRow::Folder { .. } => None,
            })
            .collect();
        assert_eq!(shown, [false, true, false, false, true]);
        // A folder's path is all of the folders it's in.
        assert!(rows.contains(&PathRow::Folder {
            depth: 1,
            name: "S02".into(),
            path: "Show/S02".into(),
            data_source_name: None,
        }));
    }

    #[test]
    fn rows_continue_the_folders_the_previous_ones_ended_in() {
        let files = [("lib", "Show/S01/e02.mkv"), ("lib", "Show/S02/e01.mkv")];
        let rows = path_rows(&files, Some(("lib", "Show/S01/e01.mkv")));
        assert_eq!(
            summary(&files, &rows),
            [
                (2, "Show/S01/e02.mkv".to_string()),
                (1, "S02/".to_string()),
                (2, "Show/S02/e01.mkv".to_string()),
            ]
        );
    }

    #[test]
    fn files_are_ordered_by_path_ignoring_case() {
        let mut files = [
            ("b", "show/b.mkv"),
            ("a", "Show/c.mkv"),
            ("a", "Show/A.mkv"),
        ];
        files.sort_by(|a, b| path_order(*a, *b));
        assert_eq!(
            files,
            [
                ("a", "Show/A.mkv"),
                ("b", "show/b.mkv"),
                ("a", "Show/c.mkv")
            ]
        );
    }
}

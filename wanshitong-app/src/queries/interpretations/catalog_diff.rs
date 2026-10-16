//! Comparing two catalogs, e.g. an open changeset's with the canonical one,
//! to see what accepting the changeset would change.
//!
//! Works have no identity across interpretations, but files do, so works are
//! matched by the files they share. Works that share files, directly or
//! through others, are compared as one change: a work changed, split, or
//! merged. A work that shares no files with the other catalog was added or
//! removed.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use uuid::Uuid;
use wanshitong_core::domain::{
    CatalogFile, CatalogPlacements, CatalogWork, DataSourceUuid, FilePart, FileRole,
};

/// Video files that leave or join the unidentified videos, other than those of
/// the changed works, e.g. files excluded from the catalog.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnidentifiedChange {
    /// Unidentified in the base catalog, but not in the other. Sorted by data
    /// source and path.
    pub left: Vec<CatalogFile>,
    /// Unidentified in the other catalog, but not in the base. Sorted by data
    /// source and path.
    pub joined: Vec<CatalogFile>,
}

impl UnidentifiedChange {
    pub fn is_empty(&self) -> bool {
        self.left.is_empty() && self.joined.is_empty()
    }
}

/// The files that leave or join the unidentified videos, leaving out those
/// of the works in `changes`, which show where they went.
pub(super) fn diff_unidentified(
    base: &CatalogPlacements,
    other: &CatalogPlacements,
    changes: &[WorkChange],
) -> UnidentifiedChange {
    let changed_works: HashSet<Uuid> = changes
        .iter()
        .flat_map(|change| change.before.iter().chain(&change.after))
        .map(|work| work.uuid)
        .collect();
    let in_changed_works = |catalog: &CatalogPlacements| {
        catalog
            .files
            .iter()
            .filter(|file| changed_works.contains(&catalog.works[file.work].uuid))
            .map(|file| (file.data_source_uuid, file.rel_path.clone()))
            .collect::<HashSet<FileKey>>()
    };
    let shown: HashSet<FileKey> = in_changed_works(base)
        .into_iter()
        .chain(in_changed_works(other))
        .collect();
    let keys = |files: &[CatalogFile]| -> HashSet<FileKey> {
        files
            .iter()
            .map(|file| (file.data_source_uuid, file.rel_path.clone()))
            .collect()
    };
    let (base_keys, other_keys) = (keys(&base.unidentified), keys(&other.unidentified));
    let only_in = |files: &[CatalogFile], others: &HashSet<FileKey>| {
        let mut files: Vec<CatalogFile> = files
            .iter()
            .filter(|file| {
                let key = (file.data_source_uuid, file.rel_path.clone());
                !others.contains(&key) && !shown.contains(&key)
            })
            .cloned()
            .collect();
        files.sort_by(|a, b| {
            (&a.data_source_name, &a.rel_path).cmp(&(&b.data_source_name, &b.rel_path))
        });
        files
    };
    UnidentifiedChange {
        left: only_in(&base.unidentified, &other_keys),
        joined: only_in(&other.unidentified, &base_keys),
    }
}

/// Works that share files, and how their files' placements differ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkChange {
    /// The works in the base catalog. None if they were added.
    pub before: Vec<CatalogWork>,
    /// The works in the other catalog. None if they were removed.
    pub after: Vec<CatalogWork>,
    /// The files whose placement differs, but for disc files, by path.
    pub files: Vec<FileChange>,
    /// How many disc files' placements differ, e.g. a Blu-ray's streams, which
    /// can number in the hundreds.
    pub disc_files: usize,
}

impl WorkChange {
    /// Whether it's one work, the same in both catalogs but for which files
    /// it has or what they are of it, e.g. an extra identified as its own.
    pub fn only_files_changed(&self) -> bool {
        matches!((self.before.as_slice(), self.after.as_slice()), ([before], [after]) if same_attributes(before, after))
    }

    pub fn kind(&self) -> WorkChangeKind {
        match (self.before.len(), self.after.len()) {
            (0, _) => WorkChangeKind::Added,
            (_, 0) => WorkChangeKind::Removed,
            (1, 1) => WorkChangeKind::Changed,
            (1, _) => WorkChangeKind::Split,
            (_, 1) => WorkChangeKind::Merged,
            _ => WorkChangeKind::Regrouped,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkChangeKind {
    /// Its files were in no work, e.g. unidentified.
    Added,
    /// Its files are in no work, e.g. unidentified.
    Removed,
    /// The same files, with different attributes, e.g. IDs, or placements.
    Changed,
    /// One work's files are several works'.
    Split,
    /// Several works' files are one work's.
    Merged,
    /// Several works' files are several other works'.
    Regrouped,
}

/// A file's placements in each catalog. A file of several episodes has
/// several.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    pub data_source_uuid: DataSourceUuid,
    pub data_source_name: String,
    pub rel_path: PathBuf,
    /// Indices into the change's `before` works, and what the file was of them.
    pub before: Vec<(usize, FilePart)>,
    /// Indices into the change's `after` works, and what the file is of them.
    pub after: Vec<(usize, FilePart)>,
}

type FileKey = (DataSourceUuid, PathBuf);

/// What differs between two catalogs, sorted by the works' titles. Works that
/// are the same, with the same files placed the same way, are left out.
pub(super) fn diff_catalogs(
    base: &CatalogPlacements,
    other: &CatalogPlacements,
) -> Vec<WorkChange> {
    // Union-find over both catalogs' works: base's first, then other's.
    let offset = base.works.len();
    let mut parent: Vec<usize> = (0..offset + other.works.len()).collect();
    fn find(parent: &mut [usize], mut node: usize) -> usize {
        while parent[node] != node {
            parent[node] = parent[parent[node]];
            node = parent[node];
        }
        node
    }
    let placed = |catalog: &CatalogPlacements| {
        let mut files = BTreeMap::<FileKey, (String, BTreeSet<(usize, FilePart)>)>::new();
        for file in &catalog.files {
            files
                .entry((file.data_source_uuid, file.rel_path.clone()))
                .or_insert_with(|| (file.data_source_name.clone(), BTreeSet::new()))
                .1
                .insert((file.work, file.part.clone()));
        }
        files
    };
    let base_files = placed(base);
    let other_files = placed(other);

    // Works that share a file are in one change.
    for (key, (_, before)) in &base_files {
        let Some((_, after)) = other_files.get(key) else {
            continue;
        };
        let nodes = before
            .iter()
            .map(|(work, _)| *work)
            .chain(after.iter().map(|(work, _)| offset + work));
        let mut roots = nodes.map(|node| find(&mut parent, node));
        if let Some(first) = roots.next() {
            for root in roots.collect::<Vec<_>>() {
                let root = find(&mut parent, root);
                let first = find(&mut parent, first);
                parent[root] = first;
            }
        }
    }

    // Each group's works, by side.
    let mut groups = BTreeMap::<usize, (Vec<usize>, Vec<usize>)>::new();
    for node in 0..parent.len() {
        let root = find(&mut parent, node);
        let group = groups.entry(root).or_default();
        if node < offset {
            group.0.push(node);
        } else {
            group.1.push(node - offset);
        }
    }
    let group_of_base: HashMap<usize, usize> = groups
        .iter()
        .flat_map(|(&root, (before, _))| before.iter().map(move |&work| (work, root)))
        .collect();
    let group_of_other: HashMap<usize, usize> = groups
        .iter()
        .flat_map(|(&root, (_, after))| after.iter().map(move |&work| (work, root)))
        .collect();

    // Each group's files, with their placements on each side.
    let mut group_files = BTreeMap::<usize, BTreeMap<FileKey, (String, FilePlacements)>>::new();
    for (key, (name, before)) in &base_files {
        let root = group_of_base[&before.iter().next().expect("placed").0];
        let entry = group_files
            .entry(root)
            .or_default()
            .entry(key.clone())
            .or_insert_with(|| (name.clone(), FilePlacements::default()));
        entry.1.before = before.clone();
    }
    for (key, (name, after)) in &other_files {
        let root = group_of_other[&after.iter().next().expect("placed").0];
        let entry = group_files
            .entry(root)
            .or_default()
            .entry(key.clone())
            .or_insert_with(|| (name.clone(), FilePlacements::default()));
        entry.1.after = after.clone();
    }

    let mut changes = Vec::new();
    for (root, (before_works, after_works)) in groups {
        let before: Vec<CatalogWork> = before_works
            .iter()
            .map(|&w| base.works[w].clone())
            .collect();
        let after: Vec<CatalogWork> = after_works
            .iter()
            .map(|&w| other.works[w].clone())
            .collect();
        let index = |works: &[usize], work: usize| works.iter().position(|&w| w == work).unwrap();
        let mut files = Vec::new();
        let mut disc_files = 0;
        for ((data_source_uuid, rel_path), (data_source_name, placements)) in
            group_files.remove(&root).unwrap_or_default()
        {
            let FilePlacements {
                before: file_before,
                after: file_after,
            } = placements;
            let parts = |placements: &BTreeSet<(usize, FilePart)>| {
                placements
                    .iter()
                    .map(|(_, part)| part.clone())
                    .collect::<BTreeSet<_>>()
            };
            // In a change of one work to one, a file is only worth showing if
            // what it is of the work changed.
            let one_to_one = before_works.len() == 1 && after_works.len() == 1;
            if one_to_one && parts(&file_before) == parts(&file_after) {
                continue;
            }
            let is_disc = file_before.iter().chain(&file_after).any(|(_, part)| {
                matches!(
                    part,
                    FilePart::Movie {
                        role: Some(FileRole::Disc),
                        ..
                    }
                )
            });
            if is_disc {
                disc_files += 1;
                continue;
            }
            files.push(FileChange {
                data_source_uuid,
                data_source_name,
                rel_path,
                before: file_before
                    .into_iter()
                    .map(|(work, part)| (index(&before_works, work), part))
                    .collect(),
                after: file_after
                    .into_iter()
                    .map(|(work, part)| (index(&after_works, work), part))
                    .collect(),
            });
        }
        let same_works =
            before.len() == 1 && after.len() == 1 && same_attributes(&before[0], &after[0]);
        if same_works && files.is_empty() && disc_files == 0 {
            continue;
        }
        changes.push(WorkChange {
            before,
            after,
            files,
            disc_files,
        });
    }
    changes.sort_by_key(|change| {
        let work = change.after.first().or(change.before.first());
        work.map(|work| (work.title.to_lowercase(), work.year))
    });
    changes
}

#[derive(Default)]
struct FilePlacements {
    before: BTreeSet<(usize, FilePart)>,
    after: BTreeSet<(usize, FilePart)>,
}

/// Whether two works, one in each catalog, are the same but for their UUIDs,
/// which are only valid in their own interpretations.
fn same_attributes(a: &CatalogWork, b: &CatalogWork) -> bool {
    let CatalogWork {
        kind,
        uuid: _,
        title,
        year,
        tmdb_id,
        imdb_id,
        tvdb_id,
    } = a;
    (kind, title, year, tmdb_id, imdb_id, tvdb_id)
        == (
            &b.kind, &b.title, &b.year, &b.tmdb_id, &b.imdb_id, &b.tvdb_id,
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use wanshitong_core::domain::{CatalogWorkKind, PlacedFile};

    fn movie(title: &str, tmdb_id: Option<i64>) -> CatalogWork {
        CatalogWork {
            kind: CatalogWorkKind::Movie,
            uuid: Uuid::new_v4(),
            title: title.into(),
            year: None,
            tmdb_id,
            imdb_id: None,
            tvdb_id: None,
        }
    }

    fn series(title: &str) -> CatalogWork {
        CatalogWork {
            kind: CatalogWorkKind::TvSeries,
            ..movie(title, Some(1))
        }
    }

    fn file(path: &str, work: usize, part: FilePart) -> PlacedFile {
        PlacedFile {
            data_source_uuid: DataSourceUuid(Uuid::nil()),
            data_source_name: "media".into(),
            rel_path: path.into(),
            work,
            part,
        }
    }

    const MAIN: FilePart = FilePart::Movie {
        role: Some(FileRole::Main),
        manifestation: Some(0),
        part: None,
    };
    const EXTRA: FilePart = FilePart::Movie {
        role: Some(FileRole::Extra),
        manifestation: None,
        part: None,
    };
    const DISC: FilePart = FilePart::Movie {
        role: Some(FileRole::Disc),
        manifestation: None,
        part: None,
    };

    fn catalog(works: Vec<CatalogWork>, files: Vec<PlacedFile>) -> CatalogPlacements {
        CatalogPlacements {
            works,
            files,
            unidentified: vec![],
        }
    }

    fn unidentified(path: &str) -> CatalogFile {
        CatalogFile {
            data_source_uuid: DataSourceUuid(Uuid::nil()),
            data_source_name: "media".into(),
            rel_path: path.into(),
        }
    }

    fn paths(files: &[CatalogFile]) -> Vec<String> {
        files
            .iter()
            .map(|file| file.rel_path.display().to_string())
            .collect()
    }

    /// Each change's kind, titles, and changed files' paths.
    fn summary(changes: &[WorkChange]) -> Vec<(WorkChangeKind, Vec<String>, Vec<String>, usize)> {
        changes
            .iter()
            .map(|change| {
                let titles = change
                    .before
                    .iter()
                    .chain(&change.after)
                    .map(|work| work.title.clone())
                    .collect();
                let files = change
                    .files
                    .iter()
                    .map(|file| file.rel_path.display().to_string())
                    .collect();
                (change.kind(), titles, files, change.disc_files)
            })
            .collect()
    }

    #[test]
    fn files_leave_or_join_the_unidentified_videos_unless_a_change_shows_them() {
        let mut before = catalog(vec![], vec![]);
        before.unidentified = vec![
            unidentified("sample.mkv"),
            unidentified("heat.mkv"),
            unidentified("same.mkv"),
        ];
        let mut after = catalog(
            vec![movie("Heat", Some(949))],
            vec![file("heat.mkv", 0, MAIN)],
        );
        after.unidentified = vec![unidentified("same.mkv"), unidentified("new.mkv")];
        let changes = diff_catalogs(&before, &after);
        // The sample was excluded, and a file that was in no work is now
        // unidentified. Identified, Heat's file is in its change instead.
        let unidentified = diff_unidentified(&before, &after, &changes);
        assert_eq!(paths(&unidentified.left), ["sample.mkv"]);
        assert_eq!(paths(&unidentified.joined), ["new.mkv"]);
    }

    #[test]
    fn the_same_catalog_has_no_changes() {
        let a = catalog(
            vec![movie("Heat", Some(949))],
            vec![file("heat.mkv", 0, MAIN), file("trailer.mkv", 0, EXTRA)],
        );
        // Interpreted again, with new UUIDs and in another order.
        let b = catalog(
            vec![movie("Heat", Some(949))],
            vec![file("trailer.mkv", 0, EXTRA), file("heat.mkv", 0, MAIN)],
        );
        assert_eq!(diff_catalogs(&a, &b), []);
    }

    #[test]
    fn identifying_files_adds_a_work_and_unidentifying_them_removes_it() {
        let none = catalog(vec![], vec![]);
        let heat = catalog(
            vec![movie("Heat", Some(949))],
            vec![file("heat.mkv", 0, MAIN), file("trailer.mkv", 0, EXTRA)],
        );
        let added = diff_catalogs(&none, &heat);
        assert_eq!(
            summary(&added),
            [(
                WorkChangeKind::Added,
                vec!["Heat".into()],
                vec!["heat.mkv".into(), "trailer.mkv".into()],
                0
            )]
        );
        assert_eq!(added[0].files[0].before, []);
        assert_eq!(added[0].files[0].after, [(0, MAIN)]);

        assert_eq!(
            summary(&diff_catalogs(&heat, &none)),
            [(
                WorkChangeKind::Removed,
                vec!["Heat".into()],
                vec!["heat.mkv".into(), "trailer.mkv".into()],
                0
            )]
        );
    }

    #[test]
    fn a_work_with_the_same_files_changes_its_attributes_or_their_parts() {
        let a = catalog(
            vec![movie("Submarine D-1", Some(162785))],
            vec![
                file("d1/index.bdmv", 0, MAIN),
                file("d1/00001.m2ts", 0, DISC),
            ],
        );
        // Another ID: only the work changes, as its files are the same.
        let b = catalog(
            vec![movie("Untold History", Some(1))],
            vec![
                file("d1/index.bdmv", 0, MAIN),
                file("d1/00001.m2ts", 0, DISC),
            ],
        );
        let changes = diff_catalogs(&a, &b);
        assert_eq!(
            summary(&changes),
            [(
                WorkChangeKind::Changed,
                vec!["Submarine D-1".into(), "Untold History".into()],
                vec![],
                0
            )]
        );
        assert!(!changes[0].only_files_changed());

        // A file becomes an extra.
        let c = catalog(
            vec![movie("Submarine D-1", Some(162785))],
            vec![
                file("d1/index.bdmv", 0, EXTRA),
                file("d1/00001.m2ts", 0, DISC),
            ],
        );
        let changes = diff_catalogs(&a, &c);
        assert_eq!(summary(&changes)[0].2, vec!["d1/index.bdmv".to_string()]);
        assert_eq!(changes[0].files[0].before, [(0, MAIN)]);
        assert_eq!(changes[0].files[0].after, [(0, EXTRA)]);
        assert!(changes[0].only_files_changed());
    }

    #[test]
    fn disc_files_are_counted_rather_than_listed() {
        let none = catalog(vec![], vec![]);
        let disc = catalog(
            vec![movie("Do the Right Thing", Some(925))],
            (0..300)
                .map(|n| file(&format!("BDMV/STREAM/{n:05}.m2ts"), 0, DISC))
                .chain([file("BDMV/index.bdmv", 0, MAIN)])
                .collect(),
        );
        assert_eq!(
            summary(&diff_catalogs(&none, &disc)),
            [(
                WorkChangeKind::Added,
                vec!["Do the Right Thing".into()],
                vec!["BDMV/index.bdmv".into()],
                300
            )]
        );
    }

    #[test]
    fn works_that_share_files_split_or_merge() {
        let one = catalog(
            vec![movie("Perry Mason", None)],
            vec![file("m-01.avi", 0, MAIN), file("m-02.avi", 0, MAIN)],
        );
        let two = catalog(
            vec![
                movie("Perry Mason Returns", Some(80809)),
                movie("The Case of the Notorious Nun", Some(226686)),
            ],
            vec![file("m-01.avi", 0, MAIN), file("m-02.avi", 1, MAIN)],
        );
        let split = diff_catalogs(&one, &two);
        assert_eq!(split[0].kind(), WorkChangeKind::Split);
        assert_eq!(split[0].files[1].after, [(1, MAIN)]);
        assert_eq!(diff_catalogs(&two, &one)[0].kind(), WorkChangeKind::Merged);
    }

    #[test]
    fn a_movie_that_becomes_episodes_of_a_series_is_one_change() {
        let a = catalog(
            vec![movie("Taskmaster's New Year Treat", None)],
            vec![file("treat.mkv", 0, MAIN)],
        );
        let b = catalog(
            vec![series("Taskmaster")],
            vec![file(
                "treat.mkv",
                0,
                FilePart::Episode {
                    season: Some(0),
                    episode: 1,
                },
            )],
        );
        let changes = diff_catalogs(&a, &b);
        assert_eq!(changes[0].kind(), WorkChangeKind::Changed);
        assert_eq!(changes[0].before[0].kind, CatalogWorkKind::Movie);
        assert_eq!(changes[0].after[0].kind, CatalogWorkKind::TvSeries);
        assert_eq!(changes[0].files.len(), 1);
    }
}

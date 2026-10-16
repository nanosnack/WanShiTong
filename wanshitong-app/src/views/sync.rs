//! Synchronizing a directory tree of symlinks with a plan.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, Permissions};
use std::io;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use wanshitong_core::domain::ViewSyncStats;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("couldn't sync {}", path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error(
        "{} is read-only to Wan Shi Tong; if it runs as the NixOS service, add the view's root path, or a directory containing it, to services.wanshitong.viewPaths",
        .0.display()
    )]
    ReadOnly(PathBuf),
    #[error(
        "{} is not a symlink or directory; views only manage their own symlinks, so move it out of the way",
        .0.display()
    )]
    UnexpectedEntry(PathBuf),
    #[error("view sync task failed: {0}")]
    Task(String),
}

/// Makes `root` hold exactly the symlinks in `desired` (link paths relative to
/// `root`, mapped to their targets), touching only what differs.
///
/// Only symlinks and directories are ever removed. If the tree holds anything
/// else, nothing is changed and the sync fails.
///
/// Directories are made readable by everyone, whatever the umask, as the view
/// is for another program (and often another user) to read.
pub(crate) fn sync_symlink_tree(
    root: &Path,
    desired: &BTreeMap<PathBuf, PathBuf>,
) -> Result<ViewSyncStats, SyncError> {
    let mut stats = ViewSyncStats::default();

    create_dirs(root)?;
    let existing = existing_links(root)?;

    // Remove first, so a stale link can't be in the way of a new directory.
    for rel_path in existing.keys() {
        if !desired.contains_key(rel_path) {
            let path = root.join(rel_path);
            fs::remove_file(&path).map_err(io_err(&path))?;
            stats.links_removed += 1;
        }
    }
    remove_empty_dirs(root)?;

    for (rel_path, target) in desired {
        let path = root.join(rel_path);
        match existing.get(rel_path) {
            Some(existing_target) if existing_target == target => {}
            Some(_) => {
                // Swap the link atomically, so it's never missing.
                let tmp_path = tmp_link_path(&path);
                symlink(target, &tmp_path).map_err(io_err(&tmp_path))?;
                fs::rename(&tmp_path, &path).map_err(io_err(&path))?;
                stats.links_updated += 1;
            }
            None => {
                if let Some(parent) = path.parent() {
                    create_dirs(parent)?;
                }
                symlink(target, &path).map_err(io_err(&path))?;
                stats.links_created += 1;
            }
        }
    }

    Ok(stats)
}

/// Removes a tree made by [`sync_symlink_tree`], `root` included, returning
/// how many links it held. A missing `root` has nothing to remove.
///
/// As with syncing, anything but symlinks and directories stops the removal
/// before anything is changed.
pub(crate) fn remove_symlink_tree(root: &Path) -> Result<u64, SyncError> {
    match fs::symlink_metadata(root) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return Err(SyncError::UnexpectedEntry(root.to_owned())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(io_err(root)(e)),
    }
    let stats = sync_symlink_tree(root, &BTreeMap::new())?;
    fs::remove_dir(root).map_err(io_err(root))?;
    Ok(stats.links_removed)
}

/// All symlinks under `root`, relative to it, with their targets.
fn existing_links(root: &Path) -> Result<HashMap<PathBuf, PathBuf>, SyncError> {
    let mut links = HashMap::new();
    for entry in walkdir::WalkDir::new(root).min_depth(1).follow_links(false) {
        let entry = entry.map_err(|e| {
            let path = e.path().unwrap_or(root).to_owned();
            SyncError::Io {
                path,
                source: e.into(),
            }
        })?;
        let file_type = entry.file_type();
        if file_type.is_symlink() {
            let target = fs::read_link(entry.path()).map_err(io_err(entry.path()))?;
            let rel_path = entry
                .path()
                .strip_prefix(root)
                .expect("walked paths are under the root")
                .to_owned();
            links.insert(rel_path, target);
        } else if !file_type.is_dir() {
            return Err(SyncError::UnexpectedEntry(entry.into_path()));
        }
    }
    Ok(links)
}

/// Removes empty directories under `root`, deepest first, but not `root`.
fn remove_empty_dirs(root: &Path) -> Result<(), SyncError> {
    let dirs = walkdir::WalkDir::new(root)
        .min_depth(1)
        .follow_links(false)
        .contents_first(true)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_dir());
    for dir in dirs {
        let is_empty = fs::read_dir(dir.path())
            .map_err(io_err(dir.path()))?
            .next()
            .is_none();
        if is_empty {
            fs::remove_dir(dir.path()).map_err(io_err(dir.path()))?;
        }
    }
    Ok(())
}

/// Like [`fs::create_dir_all`], but directories it creates are `rwxr-xr-x`.
fn create_dirs(path: &Path) -> Result<(), SyncError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => return Ok(()),
        Ok(_) => return Err(SyncError::UnexpectedEntry(path.to_owned())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(io_err(path)(e)),
    }
    if let Some(parent) = path.parent() {
        create_dirs(parent)?;
    }
    fs::create_dir(path).map_err(io_err(path))?;
    fs::set_permissions(path, Permissions::from_mode(0o755)).map_err(io_err(path))?;
    Ok(())
}

/// A sibling of `path` to build a replacement link in. One left behind by a
/// crash is not in any plan, so the next sync removes it.
fn tmp_link_path(path: &Path) -> PathBuf {
    let mut name = std::ffi::OsString::from(".");
    name.push(path.file_name().unwrap_or_default());
    name.push(".wst-tmp");
    path.with_file_name(name)
}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> SyncError + '_ {
    move |source| {
        if source.kind() == io::ErrorKind::ReadOnlyFilesystem {
            SyncError::ReadOnly(path.to_owned())
        } else {
            SyncError::Io {
                path: path.to_owned(),
                source,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;
    use tempfile::tempdir;

    fn plan(pairs: &[(&str, &str)]) -> BTreeMap<PathBuf, PathBuf> {
        pairs
            .iter()
            .map(|(link, target)| (link.into(), target.into()))
            .collect()
    }

    /// Every entry under `root`: symlinks with their targets, directories with
    /// a trailing slash, anything else as is.
    fn tree(root: &Path) -> Vec<String> {
        let mut entries: Vec<String> = walkdir::WalkDir::new(root)
            .min_depth(1)
            .into_iter()
            .map(|entry| {
                let entry = entry.unwrap();
                let rel = entry
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .display()
                    .to_string();
                if entry.path_is_symlink() {
                    format!(
                        "{rel} -> {}",
                        fs::read_link(entry.path()).unwrap().display()
                    )
                } else if entry.file_type().is_dir() {
                    format!("{rel}/")
                } else {
                    rel
                }
            })
            .collect();
        entries.sort();
        entries
    }

    fn stats(links_created: u64, links_updated: u64, links_removed: u64) -> ViewSyncStats {
        ViewSyncStats {
            links_created,
            links_updated,
            links_removed,
        }
    }

    #[test]
    fn builds_the_tree_then_does_nothing_when_rerun() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("view/Movies");
        let desired = plan(&[
            ("Heat/Heat.mkv", "/m/heat.mkv"),
            ("Heat/extras/Trailer.mkv", "/m/trailer.mkv"),
            ("Blade/Blade.mkv", "/m/blade.mkv"),
        ]);

        assert_eq!(sync_symlink_tree(&root, &desired).unwrap(), stats(3, 0, 0));
        assert_eq!(
            tree(&root),
            [
                "Blade/",
                "Blade/Blade.mkv -> /m/blade.mkv",
                "Heat/",
                "Heat/Heat.mkv -> /m/heat.mkv",
                "Heat/extras/",
                "Heat/extras/Trailer.mkv -> /m/trailer.mkv",
            ],
        );

        assert_eq!(sync_symlink_tree(&root, &desired).unwrap(), stats(0, 0, 0));
    }

    #[test]
    fn retargets_removes_and_prunes() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("Movies");
        sync_symlink_tree(
            &root,
            &plan(&[
                ("Heat/Heat.mkv", "/m/heat.mkv"),
                ("Heat/extras/Trailer.mkv", "/m/trailer.mkv"),
                ("Blade/Blade.mkv", "/m/blade.mkv"),
            ]),
        )
        .unwrap();

        let result = sync_symlink_tree(
            &root,
            &plan(&[
                ("Heat/Heat.mkv", "/new/heat.mkv"),
                ("Ronin/Ronin.mkv", "/m/ronin.mkv"),
            ]),
        )
        .unwrap();

        assert_eq!(result, stats(1, 1, 2));
        assert_eq!(
            tree(&root),
            [
                "Heat/",
                "Heat/Heat.mkv -> /new/heat.mkv",
                "Ronin/",
                "Ronin/Ronin.mkv -> /m/ronin.mkv",
            ],
        );
    }

    #[test]
    fn links_directories_without_looking_inside_them() {
        let tmp = tempdir().unwrap();
        let disc = tmp.path().join("media/Heat/VIDEO_TS (13)");
        fs::create_dir_all(&disc).unwrap();
        fs::write(disc.join("VIDEO_TS.IFO"), "").unwrap();
        let root = tmp.path().join("Movies");
        let desired = BTreeMap::from([(PathBuf::from("Heat/VIDEO_TS"), disc.clone())]);

        assert_eq!(sync_symlink_tree(&root, &desired).unwrap(), stats(1, 0, 0));
        assert_eq!(sync_symlink_tree(&root, &desired).unwrap(), stats(0, 0, 0));
        assert!(root.join("Heat/VIDEO_TS/VIDEO_TS.IFO").exists());

        assert_eq!(remove_symlink_tree(&root).unwrap(), 1);
        // The disc itself is untouched.
        assert!(disc.join("VIDEO_TS.IFO").exists());
    }

    #[test]
    fn replaces_a_link_with_a_directory() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("Movies");
        sync_symlink_tree(&root, &plan(&[("Heat", "/m/heat.mkv")])).unwrap();

        let result = sync_symlink_tree(&root, &plan(&[("Heat/Heat.mkv", "/m/heat.mkv")]));

        assert_eq!(result.unwrap(), stats(1, 0, 1));
        assert_eq!(tree(&root), ["Heat/", "Heat/Heat.mkv -> /m/heat.mkv"]);
    }

    #[test]
    fn refuses_to_touch_anything_but_symlinks_and_directories() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("Movies");
        sync_symlink_tree(&root, &plan(&[("Heat/Heat.mkv", "/m/heat.mkv")])).unwrap();
        fs::write(root.join("Heat/notes.txt"), "mine").unwrap();

        let result = sync_symlink_tree(&root, &plan(&[]));

        assert!(matches!(
            result,
            Err(SyncError::UnexpectedEntry(path)) if path == root.join("Heat/notes.txt")
        ));
        assert_eq!(
            tree(&root),
            ["Heat/", "Heat/Heat.mkv -> /m/heat.mkv", "Heat/notes.txt"],
        );
    }

    #[test]
    fn creates_directories_readable_by_everyone() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("view/Movies");
        sync_symlink_tree(&root, &plan(&[("Heat/Heat.mkv", "/m/heat.mkv")])).unwrap();

        for dir in [tmp.path().join("view"), root.clone(), root.join("Heat")] {
            let mode = fs::metadata(&dir).unwrap().mode() & 0o777;
            assert_eq!(mode, 0o755, "{}", dir.display());
        }
    }

    #[test]
    fn removes_the_whole_tree() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("view/Movies");
        sync_symlink_tree(
            &root,
            &plan(&[
                ("Heat/Heat.mkv", "/m/heat.mkv"),
                ("Heat/extras/Trailer.mkv", "/m/trailer.mkv"),
            ]),
        )
        .unwrap();

        assert_eq!(remove_symlink_tree(&root).unwrap(), 2);
        assert!(!root.exists());
        // The view's root is left for whoever made it.
        assert!(tmp.path().join("view").exists());

        assert_eq!(remove_symlink_tree(&root).unwrap(), 0);
    }

    #[test]
    fn refuses_to_remove_a_tree_holding_other_files() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("Movies");
        sync_symlink_tree(&root, &plan(&[("Heat/Heat.mkv", "/m/heat.mkv")])).unwrap();
        fs::write(root.join("Heat/notes.txt"), "mine").unwrap();

        assert!(matches!(
            remove_symlink_tree(&root),
            Err(SyncError::UnexpectedEntry(_))
        ));
        assert_eq!(
            tree(&root),
            ["Heat/", "Heat/Heat.mkv -> /m/heat.mkv", "Heat/notes.txt"],
        );
    }
}

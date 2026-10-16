use std::{
    fs,
    path::{Path, PathBuf},
};

/// At most this many completions are offered; typing more narrows them.
const MAX_COMPLETIONS: usize = 50;

/// Completions for a partly typed absolute directory path, for fields like a
/// data source's root path: the directories in the directory being typed whose
/// names start with the last, unfinished component (ignoring case). Each is the
/// whole path with a trailing `/`, so accepting one leaves the field ready for
/// the next level. Only absolute paths complete.
///
/// Anything this process can't see is left out rather than failing: a
/// directory it may not list completes to nothing, and an entry it can't
/// inspect (such as a broken symlink) is skipped. Hidden directories are only
/// offered once the component being typed starts with a `.`.
pub async fn complete_directory_path(prefix: &str) -> Vec<String> {
    // Nothing typed yet completes to nothing too. Relative paths depend on
    // the server's working directory, which means nothing to someone
    // filling in a form.
    if !prefix.starts_with('/') {
        return Vec::new();
    }
    let (dir, partial) = prefix.split_at(prefix.rfind('/').map_or(0, |slash| slash + 1));
    let (dir, partial) = (dir.to_owned(), partial.to_owned());

    tokio::task::spawn_blocking(move || {
        let mut names = matching_directories(Path::new(&dir), &partial);
        names.sort_by_cached_key(|name| name.to_lowercase());
        names.truncate(MAX_COMPLETIONS);
        names
            .into_iter()
            .map(|name| format!("{dir}{name}/"))
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// The names of the directories in `dir` that complete `partial`.
fn matching_directories(dir: &Path, partial: &str) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let partial = partial.to_lowercase();
    entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            // A name that isn't UTF-8 couldn't be typed into the field anyway.
            let name = entry.file_name().into_string().ok()?;
            if name.starts_with('.') && !partial.starts_with('.') {
                return None;
            }
            if !name.to_lowercase().starts_with(&partial) {
                return None;
            }
            is_directory(entry.path(), entry.file_type().ok()?).then_some(name)
        })
        .collect()
}

/// Whether the entry at `path` is a directory, or a symlink to one.
fn is_directory(path: PathBuf, file_type: fs::FileType) -> bool {
    if file_type.is_symlink() {
        fs::metadata(path).is_ok_and(|metadata| metadata.is_dir())
    } else {
        file_type.is_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use tempfile::tempdir;

    fn root(dir: &Path) -> String {
        dir.to_str().unwrap().to_owned()
    }

    #[tokio::test]
    async fn completes_directories_in_the_directory_being_typed() {
        let tmp = tempdir().unwrap();
        let root = root(tmp.path());
        for dir in ["Movies", "music", "Shows", ".hidden"] {
            fs::create_dir(tmp.path().join(dir)).unwrap();
        }
        fs::write(tmp.path().join("movie.mkv"), "").unwrap();
        symlink(tmp.path().join("Shows"), tmp.path().join("mshows")).unwrap();
        symlink(tmp.path().join("gone"), tmp.path().join("mbroken")).unwrap();

        assert_eq!(
            complete_directory_path(&format!("{root}/m")).await,
            [
                format!("{root}/Movies/"),
                format!("{root}/mshows/"),
                format!("{root}/music/"),
            ],
        );
        assert_eq!(
            complete_directory_path(&format!("{root}/")).await.len(),
            4,
            "everything but the file, the broken symlink and the hidden directory",
        );
        assert_eq!(
            complete_directory_path(&format!("{root}/.")).await,
            [format!("{root}/.hidden/")],
        );
    }

    #[tokio::test]
    async fn completes_nothing_it_cannot_list() {
        let tmp = tempdir().unwrap();
        let root = root(tmp.path());
        let locked = tmp.path().join("locked");
        fs::create_dir_all(locked.join("inside")).unwrap();
        fs::create_dir(tmp.path().join("open")).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

        // Root can list it anyway, so only check where permissions apply.
        if fs::read_dir(&locked).is_err() {
            assert!(
                complete_directory_path(&format!("{root}/locked/"))
                    .await
                    .is_empty()
            );
        }
        // Its siblings still complete, and so does its own name.
        assert_eq!(complete_directory_path(&format!("{root}/")).await.len(), 2);
        assert!(
            complete_directory_path(&format!("{root}/missing/"))
                .await
                .is_empty()
        );
        assert!(complete_directory_path("relative/path").await.is_empty());
        assert!(complete_directory_path("").await.is_empty());

        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

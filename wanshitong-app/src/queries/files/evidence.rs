//! The evidence a work and a file have in common: files as an
//! interpretation saw them, the top directories they are in, and the assertions
//! about them.

use crate::{AppContext, AppError};
use std::collections::HashSet;
use wanshitong_core::domain::PathId;
use wanshitong_core::domain::{
    Assertion, DataSourceUuid, DirectoryFile, FileDetails, InterpretationRowId,
};
use wanshitong_core::queries as core;

/// Files as an interpretation saw them, with their top directories and the
/// assertions about them.
pub(in crate::queries) struct FilesEvidence {
    pub(in crate::queries) files: Vec<FileDetails>,
    pub(in crate::queries) top_directories: Vec<TopDirectory>,
    pub(in crate::queries) assertions: Vec<Assertion>,
}

pub(in crate::queries) async fn files_evidence(
    ctx: &AppContext,
    interpretation_id: InterpretationRowId,
    paths: &[PathId],
) -> Result<FilesEvidence, AppError> {
    let files = core::list_file_details(&ctx.core, interpretation_id, paths).await?;

    let top_directories = top_directories(ctx, interpretation_id, &files).await?;

    let paths: HashSet<_> = paths.iter().collect();
    let assertions = core::list_interpretation_assertions(&ctx.core, interpretation_id)
        .await?
        .into_iter()
        .filter(|assertion| {
            assertion
                .files
                .iter()
                .any(|file| paths.contains(&PathId::new(file.data_source_uuid, &file.rel_path)))
        })
        .collect();

    Ok(FilesEvidence {
        files,
        top_directories,
        assertions,
    })
}

/// A directory directly below a data source's root, e.g. `Title (2001)/`,
/// with every file in it. What the rest of a top directory became is often the
/// best evidence of what a file is.
pub struct TopDirectory {
    pub data_source_uuid: DataSourceUuid,
    pub data_source_name: String,
    /// The directory itself.
    pub root: Directory,
}

pub struct Directory {
    pub name: String,
    /// Whether any of the work's own files are directly in it.
    pub has_work_files: bool,
    /// The files directly in it, in order of their names.
    pub files: Vec<DirectoryFile>,
    /// In order of their names.
    pub directories: Vec<Directory>,
}

impl Directory {
    fn new(name: String) -> Self {
        Self {
            name,
            has_work_files: false,
            files: Vec::new(),
            directories: Vec::new(),
        }
    }

    /// Adds a file at `components` below this directory.
    fn insert(&mut self, components: &[String], file: DirectoryFile, is_work_file: bool) {
        match components {
            [] | [_] => {
                self.has_work_files |= is_work_file;
                self.files.push(file);
            }
            [dir, rest @ ..] => {
                let index = match self.directories.iter().position(|d| &d.name == dir) {
                    Some(index) => index,
                    None => {
                        self.directories.push(Directory::new(dir.clone()));
                        self.directories.len() - 1
                    }
                };
                self.directories[index].insert(rest, file, is_work_file);
            }
        }
    }
}

/// The top directories that `files` are in, with all their files.
async fn top_directories(
    ctx: &AppContext,
    interpretation_id: InterpretationRowId,
    files: &[FileDetails],
) -> Result<Vec<TopDirectory>, AppError> {
    let work_files: HashSet<_> = files
        .iter()
        .map(|file| (file.data_source_uuid, file.rel_path.clone()))
        .collect();
    let mut seen = HashSet::new();
    let mut top_directories = Vec::new();
    for file in files {
        let mut components = file.rel_path.components();
        let (Some(top_directory), Some(_)) = (components.next(), components.next()) else {
            continue;
        };
        let top_directory = top_directory.as_os_str().to_string_lossy().into_owned();
        if !seen.insert((file.data_source_uuid, top_directory.clone())) {
            continue;
        }
        let mut root = Directory::new(top_directory.clone());
        let directory_files = core::list_top_directory_files(
            &ctx.core,
            interpretation_id,
            file.data_source_uuid,
            &top_directory,
        )
        .await?;
        for release_file in directory_files {
            let components: Vec<String> = release_file
                .rel_path
                .components()
                .skip(1)
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            let is_work_file =
                work_files.contains(&(file.data_source_uuid, release_file.rel_path.clone()));
            root.insert(&components, release_file, is_work_file);
        }
        top_directories.push(TopDirectory {
            data_source_uuid: file.data_source_uuid,
            data_source_name: file.data_source_name.clone(),
            root,
        });
    }
    Ok(top_directories)
}

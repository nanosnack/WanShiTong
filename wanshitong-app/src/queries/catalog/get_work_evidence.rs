use crate::interpretations::InterpretationStatus;
use crate::queries::files::{FilesEvidence, files_evidence};
use crate::queries::{TopDirectory, get_interpretation};
use crate::{AppContext, AppError, AppErrorContext};
use std::collections::HashSet;
use uuid::Uuid;
use wanshitong_core::domain::{
    Assertion, CatalogWorkKind, FileDetails, FileRole, MovieDetails, MovieUuid, PathId,
    TvSeriesDetails, TvSeriesUuid,
};
use wanshitong_core::queries as core;

/// Everything WST knows about how it derived a work: its files, with their
/// measurements and audit logs, and the assertions about them. It's what
/// someone, or an agent, reads to identify the work.
pub struct WorkEvidence {
    /// The interpretation whose catalog the work is in.
    pub interpretation: InterpretationStatus,
    pub work: Work,
    /// The work's files, as the interpretation saw them, but for the files of its
    /// discs, which its top directories summarise.
    pub files: Vec<FileDetails>,
    /// The top directories the work's files are in: the directories directly below
    /// their data sources' roots. Files at a root have none.
    pub top_directories: Vec<TopDirectory>,
    /// The assertions the interpretation included about any of the files,
    /// newest first.
    pub assertions: Vec<Assertion>,
}

pub enum Work {
    Movie(Box<MovieDetails>),
    TvSeries(Box<TvSeriesDetails>),
}

/// The evidence for the movie or series with `uuid`, in whichever
/// interpretation's catalog it's in, or None if none has it.
pub async fn get_work_evidence(
    ctx: &AppContext,
    uuid: Uuid,
) -> Result<Option<WorkEvidence>, AppError> {
    let Some(found) = core::find_work(&ctx.core, uuid).await? else {
        return Ok(None);
    };
    let interpretation = get_interpretation(ctx, found.interpretation_uuid)
        .await?
        .ok_or(AppErrorContext::InterpretationNotFound(
            found.interpretation_uuid,
        ))?;
    let (work, paths) = match found.kind {
        CatalogWorkKind::Movie => {
            let Some(movie) = core::get_movie(&ctx.core, MovieUuid(uuid)).await? else {
                return Ok(None);
            };
            // A disc's files, which can number in the hundreds, are summarised by
            // its top directory instead.
            let mut paths: Vec<_> = movie
                .files
                .iter()
                .filter(|file| file.role != Some(FileRole::Disc))
                .map(|file| PathId::new(file.data_source_uuid, &file.rel_path))
                .collect();
            paths.sort();
            (Work::Movie(Box::new(movie)), paths)
        }
        CatalogWorkKind::TvSeries => {
            let Some(series) = core::get_tv_series(&ctx.core, TvSeriesUuid(uuid)).await? else {
                return Ok(None);
            };
            let paths = series_paths(&series);
            (Work::TvSeries(Box::new(series)), paths)
        }
    };

    let FilesEvidence {
        files,
        top_directories,
        assertions,
    } = files_evidence(ctx, interpretation.interpretation.id, &paths).await?;
    Ok(Some(WorkEvidence {
        interpretation,
        work,
        files,
        top_directories,
        assertions,
    }))
}

/// A series' files, in episode order, then its extras. A file with several
/// episodes is listed once.
fn series_paths(series: &TvSeriesDetails) -> Vec<PathId> {
    let files = series
        .seasons
        .iter()
        .flat_map(|season| &season.episodes)
        .flat_map(|episode| &episode.files)
        .chain(&series.extras);
    let mut seen = HashSet::new();
    files
        .map(|file| PathId::new(file.data_source_uuid, &file.rel_path))
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

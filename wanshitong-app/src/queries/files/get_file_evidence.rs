use super::evidence::{FilesEvidence, files_evidence};
use crate::interpretations::InterpretationStatus;
use crate::queries::{TopDirectory, get_interpretation_or_canonical};
use crate::{AppContext, AppError};
use std::path::Path;
use wanshitong_core::domain::PathId;
use wanshitong_core::domain::{Assertion, DataSourceUuid, FileDetails, InterpretationUuid};

/// Everything WST knows about a file, e.g. an unidentified video: its
/// measurements and audit log, its top directory, and the assertions about it.
pub struct FileEvidence {
    /// The interpretation that saw the file.
    pub interpretation: InterpretationStatus,
    pub file: FileDetails,
    /// The top directory the file is in, unless it's at its data source's root.
    pub top_directory: Option<TopDirectory>,
    /// The assertions the interpretation included about the file, newest
    /// first.
    pub assertions: Vec<Assertion>,
}

/// The evidence for a file, as an interpretation saw it, the canonical one by
/// default, or None if it didn't see the file.
pub async fn get_file_evidence(
    ctx: &AppContext,
    interpretation: Option<InterpretationUuid>,
    data_source_uuid: DataSourceUuid,
    rel_path: &Path,
) -> Result<Option<FileEvidence>, AppError> {
    let interpretation = get_interpretation_or_canonical(ctx, interpretation).await?;
    let paths = [PathId::new(data_source_uuid, rel_path)];
    let FilesEvidence {
        files,
        top_directories,
        assertions,
    } = files_evidence(ctx, interpretation.interpretation.id, &paths).await?;
    let Some(file) = files.into_iter().next() else {
        return Ok(None);
    };
    Ok(Some(FileEvidence {
        interpretation,
        file,
        top_directory: top_directories.into_iter().next(),
        assertions,
    }))
}

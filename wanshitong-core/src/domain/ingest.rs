mod db_sink;

pub use db_sink::DbIngestSink;

use crate::{
    CoreError,
    domain::{DataSourceConfig, DataSourceUuid, FileMeasurement, FileMediaMeasurements},
};
use derive_more::Display;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Display, PartialEq, Eq, Hash, Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct IngestUuid(pub Uuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct IngestRowId(pub i64);

#[derive(Clone, Copy)]
pub struct Ingest {
    pub id: IngestRowId,
    pub uuid: IngestUuid,
    pub started_at: Timestamp,
    pub stats: IngestStats,
    pub stopped_at: Option<Timestamp>,
    pub success: bool,
}

impl Ingest {
    /// How the ingest went: pending until it stopped, as recorded.
    pub fn outcome(&self) -> IngestOutcome {
        if self.stopped_at.is_some() {
            if self.success {
                IngestOutcome::Success
            } else {
                IngestOutcome::Failed
            }
        } else {
            IngestOutcome::Pending
        }
    }
}

/// An ingest with the data source it measured.
pub struct IngestWithDataSource {
    pub ingest: Ingest,
    pub data_source: IngestDataSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IngestDataSource {
    pub uuid: DataSourceUuid,
    pub name: String,
    pub created_at: Timestamp,
    pub config: DataSourceConfig,
}

#[derive(Display)]
pub enum IngestOutcome {
    /// NOTE: It's possible for ingest jobs to finish creating measurements
    /// and then fail to mark themselves as completed in SQL. Those will stay
    /// pending forever.
    Pending,
    Failed,
    Success,
}

#[derive(Clone, Copy)]
pub struct IngestStats {
    pub files_seen: u64,
    pub files_probed: u64,
    pub files_recorded: u64,
    pub media_files_recorded: u64,
    /// Files seen but not measured: see [`IngestFileFailure`].
    pub files_failed: u64,
}

/// An `IngestSink` receives measurements and associates them with a single
/// `(ingest, data source)` in the database.
///
/// This trait is responsible for *policy and persistence*. Implementations may:
/// - Accept or reject measurements
/// - Create or update domain entities
///
/// An `IngestSink` represents the authoritative boundary between measurement
/// and domain state.
///
/// Implementations must assume that:
/// - All provided measurements are *candidates*, not pre-filtered
/// - Discovery order is not meaningful
///
/// Returning `Ok(())` indicates that the measurement was successfully handled
/// (recorded, or to be recorded on flush, or intentionally ignored according
/// to policy).
/// Returning an error indicates a failure to apply ingest logic.
///
/// This design ensures that:
/// - Ingest policy is centralized
/// - Progress reporting can accurately reflect accepted vs rejected files
/// - Probing remains reusable and policy-free
///
/// What probing a file found: its MIME type, its media measurements, and
/// ffprobe's output as given. All None for a file that wasn't probed, e.g.
/// one whose measurements are reused. An NFO's text is read whether or not
/// the file was probed, as it's small.
#[derive(Clone, Copy, Debug, Default)]
pub struct Probed<'a> {
    pub mime: Option<&'a str>,
    pub media: Option<&'a FileMediaMeasurements>,
    pub ffprobe_json: Option<&'a str>,
    /// The file's text, if it's an NFO: a release's, or a media server's.
    pub nfo_text: Option<&'a str>,
}

/// A file an ingest saw but couldn't measure. The ingest went on without it,
/// so no interpretation of the ingest sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IngestFileFailure {
    pub data_source: DataSourceUuid,
    pub rel_path: std::path::PathBuf,
    pub stage: IngestFailureStage,
    pub error: String,
}

/// Where measuring a file failed.
#[derive(Clone, Copy, Debug, Display, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum IngestFailureStage {
    /// Listing the directory, or reading the file's metadata.
    #[display("walk")]
    Walk,
    /// Reading the file's contents, e.g. with ffprobe.
    #[display("probe")]
    Probe,
}

#[async_trait::async_trait]
pub trait IngestSink: Send {
    /// Takes a file's measurements, and what probing it found, if it was
    /// probed.
    async fn file(
        &mut self,
        rel_path: &Path,
        file: &FileMeasurement,
        probed: Probed<'_>,
    ) -> Result<(), CoreError>;

    /// Takes note of a file that couldn't be measured, so the ingest can go
    /// on without it and still say which files it's missing.
    async fn file_failure(
        &mut self,
        rel_path: &Path,
        stage: IngestFailureStage,
        error: &str,
    ) -> Result<(), CoreError>;

    /// Finishes handling every measurement given to [`IngestSink::file`].
    /// Implementations may defer work, e.g. to persist in batches, so callers
    /// must flush before treating the ingest as complete.
    async fn flush(&mut self) -> Result<(), CoreError>;
}

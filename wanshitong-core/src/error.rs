use crate::{
    commands::CatalogError,
    domain::{
        AssertionUuid, ChangesetUuid, DataSourceUuid, IngestUuid, InterpretationUuid, ViewUuid,
    },
    validation::NameError,
};
use error_stack::Report;
use std::path::PathBuf;
use thiserror::Error;

/// A core failure: the [`CoreErrorContext`] it ended in, over the frames that
/// led there, each with its location, and a span trace and backtrace when
/// captured. Its `Display` is the context's message alone; `{:#}` adds each
/// cause after a colon, and `Debug` prints everything.
pub type CoreError = Report<CoreErrorContext>;

/// The typed cause of a core failure. Each message describes its own step
/// only; a wrapped error is a source, so it's reported once, as its own frame.
#[derive(Debug, Error)]
pub enum CoreErrorContext {
    #[error("assertion not found: {0}")]
    AssertionNotFound(AssertionUuid),

    #[error("an assertion must be about at least one file")]
    AssertionWithoutFiles,

    #[error(
        "changeset {changeset} can't retract assertion {assertion}: only an accepted assertion \
         that's still in effect, or one of the changeset's own, can be retracted"
    )]
    AssertionNotRetractable {
        changeset: ChangesetUuid,
        assertion: AssertionUuid,
    },

    #[error("changeset not found: {0}")]
    ChangesetNotFound(ChangesetUuid),

    #[error("changeset {0} isn't open: it was accepted or discarded")]
    ChangesetNotOpen(ChangesetUuid),

    #[error("an assertion's confidence must be from 0 to 1, not {0}")]
    InvalidConfidence(f32),

    #[error("changeset author {0}")]
    InvalidChangesetAuthor(NameError),

    #[error("core database error")]
    Database(#[from] sqlx::Error),

    #[error("data source not found: {0}")]
    DataSourceNotFound(DataSourceUuid),

    #[error("(data source, ingest) combination not found: ({0}, {1})")]
    DataSourceAndIngestNotFound(DataSourceUuid, IngestUuid),

    #[error("data source is deleted: {0}")]
    DataSourceDeleted(DataSourceUuid),

    #[error("ingest has not succeeded: {0}")]
    IngestNotSucceeded(IngestUuid),

    #[error("data source name already exists: {0}")]
    DataSourceNameExists(String),

    #[error(
        "data source root path {} overlaps view \"{view}\"",
        root_path.display()
    )]
    DataSourceOverlapsView { root_path: PathBuf, view: String },

    #[error("data source probe error")]
    DataSourceProbe(#[source] anyhow::Error),

    #[error(transparent)]
    Decode(#[from] DecodeError),

    #[error("invalid config: {1}")]
    InvalidConfig(#[source] serde_json::Error, String),

    #[error("data source name {0}")]
    InvalidDataSourceName(NameError),

    #[error(transparent)]
    InvalidCatalog(#[from] CatalogError),

    #[error("interpretation name {0}")]
    InvalidInterpretationName(NameError),

    #[error("view name {0}")]
    InvalidViewName(NameError),

    #[error("view root path must be absolute, without \"..\": {}", .0.display())]
    InvalidViewRootPath(PathBuf),

    #[error("core io error")]
    Io(#[from] std::io::Error),

    #[error("a mark must be of at least one file")]
    MarkWithoutFiles,

    #[error(
        "{file} is one of the files of the disc at {index}, which stands for them: \
         name the disc by its index file instead"
    )]
    PartOfDisc { file: String, index: String },

    #[error("interpretation not found: {0}")]
    InterpretationNotFound(InterpretationUuid),

    #[error(
        "interpretation {interpretation} previews changeset {changeset}, which isn't accepted, \
         so it can't be canonical; accept the changeset instead"
    )]
    InterpretationOfUnacceptedChangeset {
        interpretation: InterpretationUuid,
        changeset: ChangesetUuid,
    },

    #[error("unknown FileRole: {0}")]
    UnknownFileRole(String),

    #[error("unknown view kind: {0}")]
    UnknownViewKind(String),

    #[error("unknown changeset state: {0}")]
    UnknownChangesetState(String),

    #[error("unknown disc kind: {0}")]
    UnknownDiscKind(String),

    #[error("view name already exists: {0}")]
    ViewNameExists(String),

    #[error("view not found: {0}")]
    ViewNotFound(ViewUuid),

    #[error(
        "view root path {} overlaps data source \"{data_source}\"",
        view_root_path.display()
    )]
    ViewOverlapsDataSource {
        view_root_path: PathBuf,
        data_source: String,
    },

    #[error(
        "view root path {} overlaps view \"{view}\"",
        view_root_path.display()
    )]
    ViewOverlapsView {
        view_root_path: PathBuf,
        view: String,
    },
}

#[derive(Debug, thiserror::Error)]
#[error("failed to decode")]
pub struct DecodeError(#[source] pub anyhow::Error);

/// SQLite's extended result code for a UNIQUE constraint failing.
const SQLITE_CONSTRAINT_UNIQUE: &str = "2067";

pub(crate) enum UniqueViolation {
    DataSourceName,
    ViewName,
}

impl UniqueViolation {
    pub fn detect(err: &sqlx::Error) -> Option<UniqueViolation> {
        let sqlx::Error::Database(db_err) = err else {
            return None;
        };

        if db_err.code().as_deref() != Some(SQLITE_CONSTRAINT_UNIQUE) {
            return None;
        }

        let msg = db_err.message();

        match msg {
            m if m.contains("data_sources.name") => Some(UniqueViolation::DataSourceName),
            m if m.contains("views.name") => Some(UniqueViolation::ViewName),
            _ => None,
        }
    }
}

impl From<CatalogError> for CoreError {
    #[track_caller]
    fn from(error: CatalogError) -> Self {
        Report::new(CoreErrorContext::InvalidCatalog(error))
    }
}

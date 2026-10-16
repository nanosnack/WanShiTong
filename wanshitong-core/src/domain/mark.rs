use crate::domain::DataSourceUuid;
use derive_more::Display;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Display, PartialEq, Eq, Hash, Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct MarkUuid(pub Uuid);

/// A note that files need reviewing, e.g. a misidentified movie's, made while
/// browsing, to correct later. It's review state, not a claim: it never
/// changes the catalog.
#[derive(Clone, Debug)]
pub struct Mark {
    pub uuid: MarkUuid,
    /// What was marked, for display, e.g. "movie Submarine D-1".
    pub label: String,
    pub note: String,
    pub created_at: Timestamp,
    /// In order of their paths.
    pub files: Vec<MarkedFile>,
    pub progress: MarkProgress,
}

/// How far the correction of a mark's files has progressed, judged by the
/// assertions made about those files since the mark was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum MarkProgress {
    /// Nothing asserted about its files yet.
    Open,
    /// An open changeset asserts about some of its files.
    Pending,
    /// Accepted assertions about all of its files were made since it was. That
    /// doesn't mean the catalog is right now; a catalog diff says that.
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkedFile {
    pub data_source_uuid: DataSourceUuid,
    pub data_source_name: String,
    pub rel_path: PathBuf,
}

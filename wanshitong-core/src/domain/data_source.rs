use crate::domain::IngestUuid;
use derive_more::Display;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(
    Clone,
    Copy,
    Debug,
    Display,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
    Deserialize,
    Serialize,
    sqlx::Type,
)]
#[sqlx(transparent)]
pub struct DataSourceUuid(pub Uuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct DataSourceRowId(pub i64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataSource {
    pub uuid: DataSourceUuid,
    pub name: String,
    pub created_at: Timestamp,
    /// When the data source was deleted, if it was. See
    /// [`delete_data_source`](crate::commands::delete_data_source).
    pub deleted_at: Option<Timestamp>,
    pub config: DataSourceConfig,
    pub latest_ingest: Option<IngestUuid>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataSourceConfig {
    FileSystem(FileSystemSource),
}

impl DataSourceConfig {
    pub fn kind(&self) -> DataSourceKind {
        match self {
            DataSourceConfig::FileSystem(_) => DataSourceKind::FileSystem,
        }
    }
}

#[derive(Clone, Copy, Debug, Display)]
pub enum DataSourceKind {
    FileSystem,
}

impl DataSourceKind {
    pub(crate) fn as_db_str(self) -> &'static str {
        match self {
            Self::FileSystem => "filesystem",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSystemSource {
    pub root_path: PathBuf,
    pub follow_symlinks: bool,
}

/// The root relative path of a data source.
///
/// Data source relative paths do not have a leading `"/"`, so this is just an
/// empty string `""`.
pub fn root_rel_path() -> &'static Path {
    Path::new("")
}

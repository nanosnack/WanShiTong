use crate::domain::DataSourceUuid;
use std::path::PathBuf;

/// For now, we only identify files by their data source and path.
///
/// This means file renames are treated the same as a delete + create.
/// We don't track files by content, which would require expensive checksumming.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PathId {
    pub data_source: DataSourceUuid,
    pub rel_path: PathBuf,
}

impl PathId {
    pub fn new(data_source: DataSourceUuid, rel_path: impl Into<PathBuf>) -> Self {
        Self {
            data_source,
            rel_path: rel_path.into(),
        }
    }
}

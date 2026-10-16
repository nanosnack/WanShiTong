//! A file or folder as a form field or query parameter names it.

use wanshitong_app::domain::DataSourceUuid;

/// A file or folder, by its data source and path, as one value in a form
/// field or query parameter: `DATA_SOURCE|PATH`. The data source's UUID has
/// no `|`, so the first one ends it, whatever the path holds.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FileKey {
    pub(crate) data_source: DataSourceUuid,
    pub(crate) path: String,
}

impl FileKey {
    pub(crate) fn new(data_source: DataSourceUuid, path: &str) -> Self {
        Self {
            data_source,
            path: path.to_string(),
        }
    }

    /// The key as a form or query value.
    pub(crate) fn to_value(&self) -> String {
        format!("{}|{}", self.data_source, self.path)
    }

    /// The key a form or query value names, or None if it names none.
    pub(crate) fn from_value(value: &str) -> Option<Self> {
        let (data_source, path) = value.split_once('|')?;
        Some(Self {
            data_source: DataSourceUuid(data_source.parse().ok()?),
            path: path.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn a_key_survives_the_round_trip_whatever_its_path_holds() {
        let key = FileKey::new(DataSourceUuid(Uuid::new_v4()), "Show/S01|E02 (a|b).mkv");
        assert_eq!(FileKey::from_value(&key.to_value()), Some(key));
    }

    #[test]
    fn a_value_without_a_data_source_names_no_key() {
        assert_eq!(FileKey::from_value("Show/E02.mkv"), None);
        assert_eq!(FileKey::from_value("not-a-uuid|Show/E02.mkv"), None);
    }
}

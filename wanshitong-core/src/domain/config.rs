use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct ConfigSnapshotRowId(pub i64);

/// Only to be used for serialization.
#[derive(Serialize)]
pub struct VersionedConfig<T> {
    format_version: &'static str,
    config: T,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "format_version")]
pub enum AnyConfig {
    #[serde(rename = "1")]
    V1 { config: ConfigV1 },
}

impl AnyConfig {
    pub fn upgrade(self) -> ConfigV1 {
        match self {
            AnyConfig::V1 { config } => config,
        }
    }
}

pub type Config = ConfigV1;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ConfigV1 {
    pub ffprobe_path: String,
    pub ffprobe_command: String,
}

impl Default for ConfigV1 {
    fn default() -> Self {
        Self {
            ffprobe_path: "ffprobe".into(),
            ffprobe_command: [
                "-v error",
                "-print_format json",
                "-show_format",
                "-show_streams",
                "-show_chapters",
                "-show_programs",
                "-show_private_data",
                "-i",
            ]
            .join(" "),
        }
    }
}

impl VersionedConfig<ConfigV1> {
    pub const FORMAT_VERSION: &str = "1";

    pub fn new(config: ConfigV1) -> Self {
        Self {
            format_version: Self::FORMAT_VERSION,
            config,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_v1_round_trips_through_json() {
        let v1_config = ConfigV1::default();
        let json = serde_json::to_string(&VersionedConfig::new(v1_config.clone())).unwrap();
        let decoded: AnyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, AnyConfig::V1 { config: v1_config });
    }
}

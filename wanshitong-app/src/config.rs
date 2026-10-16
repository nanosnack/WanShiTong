use anyhow::Context;
use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, Ipv4Addr},
    path::{Path, PathBuf},
};
use tracing::{info, warn};
use wanshitong_core::domain::Config as CoreConfig;

/// The TMDB API Read Access Token used unless `tmdb_token` is set. It's the
/// project's, shared by every installation, and only for noncommercial use:
/// commercial users set their own (see LICENSE.md).
pub const SHARED_TMDB_TOKEN: &str = "eyJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhOTU0OTg1OGQxY2I2NjNmMDJkMDc5NTQ2NjYwOWM1OCIsIm5iZiI6MTc2ODUxMzI4My40MTYsInN1YiI6IjY5Njk1ZjAzMTQ5ODk5ZGRhNmRjODExYiIsInNjb3BlcyI6WyJhcGlfcmVhZCJdLCJ2ZXJzaW9uIjoxfQ.4SJ2EF3rmCYlQeUE7c6hdQ-nsIfi6y8_b7IgKF77e8s";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub host: IpAddr,
    pub port: u16,
    /// The TMDB API Read Access Token. A loaded config without one has
    /// [`SHARED_TMDB_TOKEN`].
    pub tmdb_token: Option<String>,
    /// Whether to show the first-run guide. Unset, the Config page decides.
    pub onboarding: Option<bool>,
    /// Data sources to create, and start ingesting, at startup. Each is
    /// skipped if any data source, even a deleted one, has its name, so once
    /// created it is the UI's like any other.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_sources: Vec<DeclaredDataSource>,
    /// Whether to ingest a data source when its files change.
    #[serde(default = "default_watch_data_sources")]
    pub watch_data_sources: bool,
    /// How long, in seconds, a data source's files must go unchanged before
    /// it's ingested, so that files added together, e.g. a season's episodes
    /// copied one after another, are ingested together. A file still being
    /// written holds the ingest back whatever this is.
    #[serde(default = "default_watch_quiet_period_secs")]
    pub watch_quiet_period_secs: u64,
    /// How many days an interpretation is kept after it was started. Every
    /// interpretation keeps a whole catalog, so older ones are deleted, but
    /// for the canonical one, any still running, and an open changeset's.
    #[serde(default = "default_interpretation_retention_days")]
    pub interpretation_retention_days: u64,
    /// The `ffprobe` to measure files with: a path, or a name found on
    /// `PATH`.
    #[serde(default = "default_ffprobe_path")]
    pub ffprobe_path: String,
    /// The arguments `ffprobe` is run with, before the file, separated by
    /// spaces. It must print JSON.
    #[serde(default = "default_ffprobe_command")]
    pub ffprobe_command: String,
}

/// A `[[data_sources]]` entry in `config.toml`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DeclaredDataSource {
    pub name: String,
    pub root_path: PathBuf,
    #[serde(default)]
    pub follow_symlinks: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: Ipv4Addr::LOCALHOST.into(),
            port: 6765, // fibonacci
            tmdb_token: None,
            onboarding: None,
            data_sources: Vec::new(),
            watch_data_sources: default_watch_data_sources(),
            watch_quiet_period_secs: default_watch_quiet_period_secs(),
            interpretation_retention_days: default_interpretation_retention_days(),
            ffprobe_path: default_ffprobe_path(),
            ffprobe_command: default_ffprobe_command(),
        }
    }
}

fn default_ffprobe_path() -> String {
    CoreConfig::default().ffprobe_path
}

fn default_ffprobe_command() -> String {
    CoreConfig::default().ffprobe_command
}

fn default_watch_data_sources() -> bool {
    true
}

fn default_watch_quiet_period_secs() -> u64 {
    5
}

fn default_interpretation_retention_days() -> u64 {
    7
}

impl Config {
    /// Loads `config.toml` from `config_dir`, with the environment over it
    /// and the defaults under it, so the file can be partial, or missing. A
    /// missing file is written with the defaults, as a template, where the
    /// directory allows it; a service whose configuration directory is
    /// root's, as systemd's `ConfigurationDirectory=` is, runs without one.
    pub fn load(config_dir: &Path) -> Result<Config, LoadConfigError> {
        let path = config_dir.join("config.toml");
        if !path.exists() {
            let template = toml::to_string_pretty(&Config::default()).unwrap();
            match std::fs::write(&path, template) {
                Ok(()) => info!("Wrote the default config to {}", path.display()),
                Err(e) => warn!(
                    "Couldn't write the default config to {}: {e}; running with the defaults",
                    path.display()
                ),
            }
        }
        let mut config: Config = Figment::from(Serialized::defaults(Config::default()))
            .merge(Toml::file(path))
            .merge(Env::prefixed("WST__"))
            .extract()
            .context("extracting config via figment")?;
        config
            .tmdb_token
            .get_or_insert_with(|| SHARED_TMDB_TOKEN.into());
        Ok(config)
    }

    /// The settings that ingests run with, which the database keeps a
    /// snapshot of.
    pub fn core(&self) -> CoreConfig {
        CoreConfig {
            ffprobe_path: self.ffprobe_path.clone(),
            ffprobe_command: self.ffprobe_command.clone(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("failed to load config")]
pub struct LoadConfigError(#[from] anyhow::Error);

#[cfg(test)]
// A `Jail` closure returns figment's `Result`, whose error is large.
#[allow(clippy::result_large_err)]
mod tests {
    use super::*;
    use figment::Jail;

    // Each test loads its config in a `Jail`: a directory of its own, and
    // the environment, which `Config::load` reads, to itself for the while.

    #[test]
    fn the_initial_config_file_names_the_probing_settings() {
        Jail::expect_with(|jail| {
            let config = Config::load(jail.directory()).unwrap();
            let toml = std::fs::read_to_string(jail.directory().join("config.toml")).unwrap();
            assert!(toml.contains("ffprobe_path = \"ffprobe\""), "{toml}");
            assert!(toml.contains("ffprobe_command = "), "{toml}");
            assert_eq!(config.core(), CoreConfig::default());
            Ok(())
        });
    }

    #[test]
    fn a_missing_or_partial_file_leaves_the_defaults() {
        Jail::expect_with(|jail| {
            let unwritable = jail.directory().join("missing");
            let config = Config::load(&unwritable).unwrap();
            assert_eq!(config.port, Config::default().port);
            assert!(!unwritable.exists());

            jail.create_file("config.toml", "port = 7000\n")?;
            let config = Config::load(jail.directory()).unwrap();
            assert_eq!(config.port, 7000);
            assert_eq!(config.host, Config::default().host);
            Ok(())
        });
    }

    #[test]
    fn the_probing_settings_come_from_the_file_then_the_environment() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "config.toml",
                "host = \"127.0.0.1\"\nport = 6765\n\
                 ffprobe_path = \"/opt/ffmpeg/bin/ffprobe\"\nffprobe_command = \"-show_streams -i\"\n",
            )?;
            let config = Config::load(jail.directory()).unwrap();
            assert_eq!(config.ffprobe_path, "/opt/ffmpeg/bin/ffprobe");
            assert_eq!(config.ffprobe_command, "-show_streams -i");

            jail.set_env("WST__FFPROBE_PATH", "/usr/local/bin/ffprobe");
            let config = Config::load(jail.directory()).unwrap();
            assert_eq!(config.ffprobe_path, "/usr/local/bin/ffprobe");
            assert_eq!(config.ffprobe_command, "-show_streams -i");
            Ok(())
        });
    }
}

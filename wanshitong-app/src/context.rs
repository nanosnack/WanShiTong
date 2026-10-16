use crate::AppErrorContext;
use crate::tmdb_client::TmdbClient;
use crate::{
    AppError, Config,
    ingest::{IngestProgressRegistry, IngestQueue},
    interpretations::RunningInterpretations,
};
use jiff::Timestamp;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Notify};
use uuid::Uuid;
use wanshitong_core::CoreContext;

#[derive(Clone)]
pub struct AppContext {
    start_instant: Instant,
    start_time: Timestamp,
    config: Config,
    cache_dir: PathBuf,

    pub(crate) core: CoreContext,
    pub(crate) ingest_progress: IngestProgressRegistry,
    pub(crate) ingest_queue: IngestQueue,
    pub(crate) tasks: Arc<Mutex<HashMap<TaskId, tokio::task::JoinHandle<()>>>>,
    pub(crate) running_interpretations: RunningInterpretations,
    /// Held while deriving an interpretation's catalog.
    pub(crate) interpreting: Arc<Mutex<()>>,
    /// Held while refreshing a view.
    pub(crate) view_refresh: Arc<Mutex<()>>,
    /// Notified when a data source is created, deleted or restored, so the
    /// watcher can watch the data sources there are now.
    pub(crate) data_sources_changed: Arc<Notify>,
}

impl AppContext {
    pub fn new(core: CoreContext, config: Config, cache_dir: impl Into<PathBuf>) -> Self {
        let ingest_progress = IngestProgressRegistry::default();
        Self {
            start_instant: Instant::now(),
            start_time: Timestamp::now(),
            config,
            cache_dir: cache_dir.into(),
            core,
            ingest_progress,
            ingest_queue: Default::default(),
            tasks: Default::default(),
            running_interpretations: Default::default(),
            interpreting: Default::default(),
            view_refresh: Default::default(),
            data_sources_changed: Default::default(),
        }
    }

    pub fn started_at(&self) -> Timestamp {
        self.start_time
    }

    pub fn uptime(&self) -> Duration {
        self.start_instant.elapsed()
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// A TMDB client, sharing the cache that interpretations use, or None if
    /// there's no TMDB token.
    pub(crate) fn tmdb_client(&self) -> Option<TmdbClient> {
        let token = self.config.tmdb_token.clone()?;
        Some(TmdbClient::new(self.cache_dir.join("http_cache"), token))
    }

    /// The TMDB client, or the error that there is no knowledge base to ask.
    pub(crate) fn tmdb(&self) -> Result<TmdbClient, AppError> {
        self.tmdb_client()
            .ok_or_else(|| AppErrorContext::NoKnowledgeBase.into())
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub(crate) struct TaskId(pub Uuid);

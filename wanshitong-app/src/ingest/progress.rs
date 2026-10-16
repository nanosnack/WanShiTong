use dashmap::DashMap;
use jiff::{SignedDuration, Timestamp};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;
use wanshitong_core::domain::{IngestStats, IngestUuid};

#[derive(Clone, Default)]
pub struct IngestProgressRegistry {
    runs: Arc<DashMap<IngestUuid, Arc<IngestProgress>>>,
}

impl IngestProgressRegistry {
    pub fn start(&self, id: IngestUuid) -> IngestProgressReporter {
        let progress = self
            .runs
            .entry(id)
            .or_insert_with(|| Arc::new(IngestProgress::start()))
            .clone();
        IngestProgressReporter { progress }
    }

    pub fn listen(&self, id: IngestUuid) -> Option<IngestProgressListener> {
        self.runs.get(&id).map(|progress| IngestProgressListener {
            progress: progress.clone(),
        })
    }
}

#[derive(Clone)]
pub struct IngestProgressListener {
    progress: Arc<IngestProgress>,
}

impl IngestProgressListener {
    pub async fn poll(&self) -> IngestProgressSnapshot {
        self.progress.snapshot().await
    }
}

#[derive(Clone)]
pub struct IngestProgressReporter {
    progress: Arc<IngestProgress>,
}

impl IngestProgressReporter {
    pub fn file_seen(&self) {
        self.progress.files_seen.fetch_add(1, Ordering::Relaxed);
    }

    pub fn file_probed(&self) {
        self.progress.files_probed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn media_file_recorded(&self) {
        self.progress
            .media_files_recorded
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn file_recorded(&self) {
        self.progress.files_recorded.fetch_add(1, Ordering::Relaxed);
    }

    pub fn file_failed(&self) {
        self.progress.files_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn stats(&self) -> IngestStats {
        IngestStats {
            files_seen: self.progress.files_seen.load(Ordering::Relaxed),
            files_probed: self.progress.files_probed.load(Ordering::Relaxed),
            files_recorded: self.progress.files_recorded.load(Ordering::Relaxed),
            media_files_recorded: self.progress.media_files_recorded.load(Ordering::Relaxed),
            files_failed: self.progress.files_failed.load(Ordering::Relaxed),
        }
    }

    pub async fn log_event(&self, event: IngestEvent) {
        let mut inner = self.progress.inner.write().await;
        inner.logged_events.push(event);
    }

    pub async fn finish(&self, error: Option<String>) {
        let now = Timestamp::now();
        let mut inner = self.progress.inner.write().await;
        if let Some(error) = &error {
            inner.logged_events.push(IngestEvent {
                name: "Fatal Error".into(),
                time: now,
                payload: error.to_string(),
            });
        }
        inner.error = error;
        inner.stopped_at = Some(now);
    }
}

pub struct IngestProgress {
    // hot-path counters
    files_seen: AtomicU64,
    files_probed: AtomicU64,
    media_files_recorded: AtomicU64,
    files_recorded: AtomicU64,
    files_failed: AtomicU64,

    inner: RwLock<IngestProgressState>,
}

impl IngestProgress {
    fn start() -> Self {
        let now = Timestamp::now();
        Self {
            files_seen: Default::default(),
            files_probed: Default::default(),
            media_files_recorded: Default::default(),
            files_recorded: Default::default(),
            files_failed: Default::default(),
            inner: RwLock::new(IngestProgressState {
                started_at: now,
                stopped_at: None,
                logged_events: Default::default(),
                error: None,
            }),
        }
    }

    async fn snapshot(&self) -> IngestProgressSnapshot {
        let inner = self.inner.read().await;
        IngestProgressSnapshot {
            files_seen: self.files_seen.load(Ordering::Relaxed),
            files_probed: self.files_probed.load(Ordering::Relaxed),
            media_files_recorded: self.media_files_recorded.load(Ordering::Relaxed),
            files_recorded: self.files_recorded.load(Ordering::Relaxed),
            files_failed: self.files_failed.load(Ordering::Relaxed),
            started_at: inner.started_at,
            stopped_at: inner.stopped_at,
            logged_events: inner.logged_events.clone(),
            error: inner.error.clone(),
        }
    }
}

struct IngestProgressState {
    started_at: Timestamp,
    stopped_at: Option<Timestamp>,
    logged_events: Vec<IngestEvent>,
    error: Option<String>,
}

pub struct IngestProgressSnapshot {
    pub files_seen: u64,
    pub files_probed: u64,
    pub media_files_recorded: u64,
    pub files_recorded: u64,
    pub files_failed: u64,

    pub started_at: Timestamp,
    pub stopped_at: Option<Timestamp>,
    pub logged_events: Vec<IngestEvent>,
    pub error: Option<String>,
}

impl IngestProgressSnapshot {
    pub fn is_finished(&self) -> bool {
        self.stopped_at.is_some()
    }

    pub fn time_elapsed(&self) -> Duration {
        let end = if let Some(stopped_at) = self.stopped_at {
            stopped_at
        } else {
            Timestamp::now()
        };
        end.duration_since(self.started_at)
            .max(SignedDuration::ZERO)
            .unsigned_abs()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IngestEvent {
    pub name: String,
    pub time: Timestamp,
    pub payload: String,
}

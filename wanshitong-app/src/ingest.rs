mod progress;
mod queue;

pub use progress::{IngestEvent, IngestProgressSnapshot};
pub use wanshitong_core::domain::{IngestUuid, IngestWithDataSource};

pub(crate) use progress::{IngestProgressRegistry, IngestProgressReporter};
pub(crate) use queue::IngestQueue;

use jiff::{SignedDuration, Timestamp};
use std::time::Duration;
use wanshitong_core::{
    self as core,
    domain::{Ingest, IngestDataSource},
};

pub struct IngestStatus {
    pub ingest: Ingest,
    pub data_source: IngestDataSource,
    pub progress: Option<IngestProgressSnapshot>,
}

impl IngestStatus {
    pub fn files_seen(&self) -> u64 {
        if let Some(progress) = &self.progress {
            progress.files_seen
        } else {
            self.ingest.stats.files_seen
        }
    }

    pub fn files_probed(&self) -> u64 {
        if let Some(progress) = &self.progress {
            progress.files_probed
        } else {
            self.ingest.stats.files_probed
        }
    }

    pub fn files_recorded(&self) -> u64 {
        if let Some(progress) = &self.progress {
            progress.files_recorded
        } else {
            self.ingest.stats.files_recorded
        }
    }

    pub fn media_files_recorded(&self) -> u64 {
        if let Some(progress) = &self.progress {
            progress.media_files_recorded
        } else {
            self.ingest.stats.media_files_recorded
        }
    }

    pub fn files_failed(&self) -> u64 {
        if let Some(progress) = &self.progress {
            progress.files_failed
        } else {
            self.ingest.stats.files_failed
        }
    }

    pub fn started_at(&self) -> Timestamp {
        if let Some(progress) = &self.progress {
            progress.started_at
        } else {
            self.ingest.started_at
        }
    }

    pub fn stopped_at(&self) -> Option<Timestamp> {
        if let Some(progress) = &self.progress {
            progress.stopped_at
        } else {
            self.ingest.stopped_at
        }
    }

    pub fn elapsed(&self) -> Duration {
        if let Some(progress) = &self.progress {
            progress.time_elapsed()
        } else if let Some(stopped_at) = self.ingest.stopped_at {
            stopped_at
                .duration_since(self.ingest.started_at)
                .max(SignedDuration::ZERO)
                .unsigned_abs()
        } else {
            Timestamp::now()
                .duration_since(self.ingest.started_at)
                .max(SignedDuration::ZERO)
                .unsigned_abs()
        }
    }

    pub fn state(&self) -> IngestState {
        if let Some(progress) = &self.progress {
            if progress.stopped_at.is_some() {
                if progress.error.is_some() {
                    IngestState::Failed
                } else {
                    IngestState::Success
                }
            } else {
                IngestState::Pending
            }
        } else {
            match self.ingest.outcome() {
                core::domain::IngestOutcome::Pending => {
                    // In this case, because there is no progress AND core
                    // thinks it's pending, we actually don't know exactly
                    // where it failed, we just know the task exited without
                    // persisting itself as complete.
                    IngestState::Failed
                }
                // TODO: it might be nice to persist the reason for failure
                core::domain::IngestOutcome::Failed => IngestState::Failed,
                core::domain::IngestOutcome::Success => IngestState::Success,
            }
        }
    }
}

pub enum IngestState {
    Pending,
    Failed,
    Success,
}

impl std::fmt::Display for IngestState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IngestState::Pending => write!(f, "Pending"),
            IngestState::Failed => write!(f, "Failed"),
            IngestState::Success => write!(f, "Success"),
        }
    }
}

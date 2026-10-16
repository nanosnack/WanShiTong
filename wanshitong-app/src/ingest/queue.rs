use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wanshitong_core::domain::DataSourceUuid;

/// The data sources being ingested, and whether another ingest of each is
/// queued. One task at a time ingests a data source, and runs its queued
/// ingest itself, so ingests of a data source never overlap.
#[derive(Clone, Default)]
pub(crate) struct IngestQueue {
    /// Whether another ingest is queued, by data source being ingested.
    ingesting: Arc<Mutex<HashMap<DataSourceUuid, bool>>>,
}

impl IngestQueue {
    /// Claims the data source for a new ingest task and returns true, or, if
    /// a task already has it, queues another ingest for that task.
    pub(crate) fn claim_or_queue(&self, source: DataSourceUuid) -> bool {
        let mut ingesting = self.ingesting.lock().unwrap();
        match ingesting.get_mut(&source) {
            Some(queued) => {
                *queued = true;
                false
            }
            None => {
                ingesting.insert(source, false);
                true
            }
        }
    }

    /// For the task that claimed the data source: takes its queued ingest
    /// and returns true, or, if none is queued, releases the data source.
    pub(crate) fn take_queued_or_release(&self, source: DataSourceUuid) -> bool {
        let mut ingesting = self.ingesting.lock().unwrap();
        match ingesting.get_mut(&source) {
            Some(queued) if *queued => {
                *queued = false;
                true
            }
            _ => {
                ingesting.remove(&source);
                false
            }
        }
    }

    /// For the task that claimed the data source: releases it, dropping any
    /// queued ingest, e.g. because the data source can't be ingested.
    pub(crate) fn release(&self, source: DataSourceUuid) {
        self.ingesting.lock().unwrap().remove(&source);
    }
}

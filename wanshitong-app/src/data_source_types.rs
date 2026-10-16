//! Concrete implementations for ingesting data from external sources.

pub mod fs;

use std::{collections::HashMap, path::PathBuf};
use wanshitong_core::{commands::PreviousMediaMeasurement, domain::IngestSink};

use crate::ingest::IngestProgressReporter;

/// A `DataSourceProbe` is responsible for generating all measurements for a
/// single data source during a single ingest.
///
/// Implementations should try to avoid reporting duplicate measurements.
///
/// `prev_media_measurements` is provided as an optimization: if the data source
/// determines a file's content hasn't changed, it can skip probing the file
/// again.
///
#[async_trait::async_trait]
pub trait DataSourceProbe {
    async fn probe(
        &self,
        progress: &IngestProgressReporter,
        prev_media_measurements: &HashMap<PathBuf, PreviousMediaMeasurement>,
        sink: &mut dyn IngestSink,
    ) -> anyhow::Result<()>;
}

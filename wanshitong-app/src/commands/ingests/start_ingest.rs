use crate::report_error;
use std::path::Path;
use wanshitong_core::domain::Probed;

use crate::commands::{create_latest_interpretation, fit_name, spawn_interpretation};
use crate::context::TaskId;
use crate::data_source_types::DataSourceProbe;
use crate::data_source_types::fs::FileSystemProbe;
use crate::file_probe::FFProbeTool;
use crate::ingest::{IngestEvent, IngestProgressReporter};
use crate::queries::get_data_source;
use crate::{AppContext, AppError};
use futures::FutureExt;
use jiff::Timestamp;
use tracing::error;
use uuid::Uuid;
use wanshitong_core::commands::{
    NewIngest, NewIngestDataSource, finish_ingest, start_ingest as start_ingest_core,
};
use wanshitong_core::domain::{
    DataSourceConfig, DataSourceUuid, DbIngestSink, FileMeasurement, IngestFailureStage,
    IngestRowId, IngestSink, IngestUuid,
};
use wanshitong_core::queries::list_data_sources;
use wanshitong_core::{CoreError, CoreErrorContext};

/// What became of a request to ingest a data source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IngestStart {
    /// This ingest started.
    Started(IngestUuid),
    /// The data source was already being ingested, so it will be ingested
    /// again once that ingest finishes.
    Queued,
}

/// Starts ingesting a data source, unless it's already being ingested: then
/// it's ingested again once that ingest finishes, since its files may have
/// changed after that ingest read them. However many requests come meanwhile,
/// one more ingest runs.
pub async fn start_ingest(
    ctx: &AppContext,
    source_id: DataSourceUuid,
) -> Result<IngestStart, AppError> {
    if !ctx.ingest_queue.claim_or_queue(source_id) {
        return Ok(IngestStart::Queued);
    }
    let ingest = match begin_ingest(ctx, source_id).await {
        Ok(ingest) => ingest,
        Err(e) => {
            ctx.ingest_queue.release(source_id);
            return Err(e);
        }
    };
    let ingest_uuid = ingest.new_ingest.uuid;
    let task_id = TaskId(Uuid::new_v4());
    // Lock task container before spawning to ensure we insert the task before
    // the spawned task tries to remove itself.
    let mut tasks = ctx.tasks.lock().await;
    let task = tokio::spawn(ingest_task(ctx.clone(), task_id, source_id, ingest));
    tasks.insert(task_id, task);
    Ok(IngestStart::Started(ingest_uuid))
}

/// An ingest that has been recorded as started, and has yet to run.
struct BegunIngest {
    new_ingest: NewIngest,
    config: DataSourceConfig,
    reporter: IngestProgressReporter,
}

async fn begin_ingest(
    ctx: &AppContext,
    source_id: DataSourceUuid,
) -> Result<BegunIngest, AppError> {
    let source = get_data_source(ctx, source_id)
        .await?
        .ok_or(CoreErrorContext::DataSourceNotFound(source_id))?;
    let new_ingest = start_ingest_core(&ctx.core, source_id).await?;
    let reporter = ctx.ingest_progress.start(new_ingest.uuid);
    Ok(BegunIngest {
        new_ingest,
        config: source.config,
        reporter,
    })
}

/// Runs the ingest, then each ingest queued meanwhile, until none is.
async fn ingest_task(
    ctx: AppContext,
    task_id: TaskId,
    source_id: DataSourceUuid,
    mut ingest: BegunIngest,
) {
    loop {
        panic_safe_ingest(&ctx, ingest).await;
        if !ctx.ingest_queue.take_queued_or_release(source_id) {
            break;
        }
        ingest = match begin_ingest(&ctx, source_id).await {
            Ok(ingest) => ingest,
            Err(e) => {
                report_error!(e, "Couldn't start a queued ingest");
                ctx.ingest_queue.release(source_id);
                break;
            }
        };
    }
    ctx.tasks.lock().await.remove(&task_id);
}

async fn panic_safe_ingest(ctx: &AppContext, ingest: BegunIngest) {
    let BegunIngest {
        new_ingest,
        config,
        reporter,
    } = ingest;
    let result =
        std::panic::AssertUnwindSafe(run_ingest_task(ctx, new_ingest, config, reporter.clone()))
            .catch_unwind()
            .await;
    match result {
        Ok(()) => {
            // On success, the reporter will already have finished.
        }
        Err(_) => {
            // Unwind error is not useful to users; give generic error message.
            reporter.finish(Some("ingest task panicked".into())).await;
        }
    }
}

async fn run_ingest_task(
    ctx: &AppContext,
    new_ingest: NewIngest,
    config: DataSourceConfig,
    reporter: IngestProgressReporter,
) {
    let NewIngest {
        row_id,
        data_source,
        ffprobe_path,
        ffprobe_command,
        ..
    } = new_ingest;
    let ffprobe_tool = FFProbeTool::new(ffprobe_path.into(), ffprobe_command);

    let db_sink = DbIngestSink::new(&ctx.core, row_id, data_source.row_id);
    let mut reporting_sink = ProgressReportingSink::new(db_sink, reporter);
    let result = run_ingest_fallible(
        ctx,
        ffprobe_tool,
        row_id,
        &data_source,
        config,
        &mut reporting_sink,
    )
    .await;
    // Started before the ingest is reported finished, so that anyone waiting
    // on the ingest already sees the interpretation running. It derives the
    // catalog in its own task, so the ingest doesn't wait for that.
    if result.is_ok()
        && let Err(e) = autogenerate_catalog(ctx, &[data_source.uuid]).await
    {
        report_error!(e, "Failed to start interpretation after ingest");
    }
    let report_err = result.err().map(|e| format!("{e:#}"));
    reporting_sink.reporter.finish(report_err).await;
}

async fn run_ingest_fallible(
    ctx: &AppContext,
    ffprobe_tool: FFProbeTool,
    ingest_id: IngestRowId,
    data_source: &NewIngestDataSource,
    data_source_config: DataSourceConfig,
    sink: &mut Sink<'_>,
) -> Result<(), AppError> {
    let reporter = sink.reporter.clone();
    let work_result = match data_source_probe(ffprobe_tool, data_source_config)
        .probe(&reporter, &data_source.prev_media_measurements, sink)
        .await
    {
        Ok(()) => sink.flush().await,
        Err(e) => Err(CoreErrorContext::DataSourceProbe(e).into()),
    };
    let stats = sink.reporter.stats();
    let completion_result = finish_ingest(&ctx.core, ingest_id, work_result.is_ok(), stats).await;
    if let Err(e) = &work_result {
        report_error!(e, "Ingest failed");
    }
    if let Err(e) = &completion_result {
        report_error!(e, "Couldn't record how the ingest ended");
    }
    // TODO: surface both errors?
    work_result?;
    completion_result?;
    Ok(())
}

async fn autogenerate_catalog(
    ctx: &AppContext,
    ingested: &[DataSourceUuid],
) -> Result<(), AppError> {
    let names: Vec<_> = list_data_sources(&ctx.core)
        .await?
        .into_iter()
        .filter(|ds| ingested.contains(&ds.uuid))
        .map(|ds| ds.name)
        .collect();
    let name = after_ingest_name(&names);
    let new_interpretation = create_latest_interpretation(ctx, &name).await?;
    spawn_interpretation(ctx, new_interpretation, true).await;
    Ok(())
}

/// "After ingest of A, B", truncated to fit the name length limit.
fn after_ingest_name(data_source_names: &[String]) -> String {
    if data_source_names.is_empty() {
        return "After ingest".into();
    }
    fit_name(format!("After ingest of {}", data_source_names.join(", ")))
}

fn data_source_probe(
    ffprobe_tool: FFProbeTool,
    config: DataSourceConfig,
) -> Box<dyn DataSourceProbe + Send> {
    match config {
        DataSourceConfig::FileSystem(fs_source) => {
            Box::new(FileSystemProbe::new(fs_source, ffprobe_tool))
        }
    }
}

type Sink<'a> = ProgressReportingSink<DbIngestSink<'a>>;

struct ProgressReportingSink<S> {
    inner: S,
    reporter: IngestProgressReporter,
}

impl<S> ProgressReportingSink<S> {
    fn new(inner: S, reporter: IngestProgressReporter) -> Self {
        Self { inner, reporter }
    }
}

#[async_trait::async_trait]
impl<S: IngestSink> IngestSink for ProgressReportingSink<S> {
    async fn file(
        &mut self,
        rel_path: &Path,
        file: &FileMeasurement,
        probed: Probed<'_>,
    ) -> Result<(), CoreError> {
        let result = self.inner.file(rel_path, file, probed).await;
        match result {
            Ok(()) => {
                self.reporter.file_recorded();
                if probed.media.is_some() || file.media_measurement_id.is_some() {
                    self.reporter.media_file_recorded();
                }
                Ok(())
            }
            Err(e) => {
                report_error!(e, "Failed to record file measurement");
                self.reporter
                    .log_event(IngestEvent {
                        name: "Failed to record file measurement".into(),
                        time: Timestamp::now(),
                        payload: format!("{e:#}"),
                    })
                    .await;
                Err(e)
            }
        }
    }

    async fn file_failure(
        &mut self,
        rel_path: &Path,
        stage: IngestFailureStage,
        error: &str,
    ) -> Result<(), CoreError> {
        error!(
            "Couldn't measure \"{}\" ({stage}): {error}",
            rel_path.display()
        );
        self.reporter.file_failed();
        self.inner.file_failure(rel_path, stage, error).await
    }

    async fn flush(&mut self) -> Result<(), CoreError> {
        let result = self.inner.flush().await;
        if let Err(e) = &result {
            report_error!(e, "Failed to record file measurements");
            self.reporter
                .log_event(IngestEvent {
                    name: "Failed to record file measurements".into(),
                    time: Timestamp::now(),
                    payload: format!("{e:#}"),
                })
                .await;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wanshitong_core::MAX_NAME_LEN;

    #[test]
    fn after_ingest_name_fits_the_name_limit() {
        let long = "é".repeat(MAX_NAME_LEN);
        let name = after_ingest_name(&[long]);
        assert!(name.len() <= MAX_NAME_LEN);
        assert!(name.starts_with("After ingest of é"));
        assert!(name.ends_with('…'));
    }
}

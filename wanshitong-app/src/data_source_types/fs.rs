use crate::{
    data_source_types::DataSourceProbe,
    file_probe::{FFProbeTool, FileProbeResult, MAX_CONCURRENT_PROBES, probe_file},
    ingest::IngestProgressReporter,
};
use std::{
    collections::HashMap,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{sync::Semaphore, task::JoinHandle};
use tracing::error;
use wanshitong_core::domain::Probed;
use wanshitong_core::{
    CoreError,
    commands::PreviousMediaMeasurement,
    domain::{
        FileMeasurement, FileMediaMeasurementRowId, FileMediaMeasurements, FileSystemSource,
        IngestFailureStage, IngestSink,
    },
};

pub struct FileSystemProbe {
    source: FileSystemSource,
    ffprobe_tool: FFProbeTool,
}

impl FileSystemProbe {
    pub fn new(source: FileSystemSource, ffprobe_tool: FFProbeTool) -> Self {
        Self {
            source,
            ffprobe_tool,
        }
    }
}

/// What the walk found: a file to measure, or a path it couldn't read. One
/// path's trouble is noted and the walk goes on; only trouble with the root
/// itself fails it, since a missing or unreadable root means the whole data
/// source is gone, e.g. unmounted, and shouldn't be ingested as empty.
enum Walked {
    File(walkdir::DirEntry, std::fs::Metadata),
    Failed(PathBuf, String),
}

#[async_trait::async_trait]
impl DataSourceProbe for FileSystemProbe {
    async fn probe(
        &self,
        progress: &IngestProgressReporter,
        prev_media_measurements: &HashMap<PathBuf, PreviousMediaMeasurement>,
        sink: &mut dyn IngestSink,
    ) -> anyhow::Result<()> {
        let Self {
            source,
            ffprobe_tool,
        } = self;

        let ffprobe_tool = Arc::new(ffprobe_tool.clone());
        // This ingest's share of the probes in flight at once; see
        // `MAX_CONCURRENT_PROBES` for why it equals the shared limit.
        let in_flight = Arc::new(Semaphore::new(MAX_CONCURRENT_PROBES));

        // walkdir is blocking so we isolate it.
        let root_path = source.root_path.clone();
        let follow_symlinks = source.follow_symlinks;
        let (walk_tx, mut walk_rx) = tokio::sync::mpsc::channel(16);
        let walk_task = tokio::task::spawn_blocking(move || {
            for entry in walkdir::WalkDir::new(&root_path).follow_links(follow_symlinks) {
                let walked = match entry {
                    Ok(entry) if entry.file_type().is_file() => match entry.metadata() {
                        Ok(meta) => Walked::File(entry, meta),
                        Err(e) => Walked::Failed(entry.into_path(), e.to_string()),
                    },
                    Ok(_) => continue,
                    Err(e) if e.depth() == 0 => return Err(e.into()),
                    Err(e) => {
                        let path = e.path().unwrap_or(&root_path).to_owned();
                        Walked::Failed(path, e.to_string())
                    }
                };
                if walk_tx.blocking_send(walked).is_err() {
                    break;
                }
            }
            anyhow::Ok(())
        });

        let mut probe_tasks = Vec::new();
        while let Some(walked) = walk_rx.recv().await {
            let (entry, meta) = match walked {
                Walked::File(entry, meta) => (entry, meta),
                Walked::Failed(path, error) => {
                    let rel_path = rel_path(&source.root_path, &path);
                    sink.file_failure(&rel_path, IngestFailureStage::Walk, &error)
                        .await
                        .map_err(CoreError::into_error)?;
                    continue;
                }
            };
            progress.file_seen();

            let path = entry.into_path();
            let rel_path = rel_path(&source.root_path, &path);

            // Avoid probing if the file content has not changed since the
            // previous media measurement.
            let media_measurement = if let Some(prev_measurement) =
                prev_media_measurements.get(&rel_path)
                && prev_measurement.mtime == meta.mtime()
                && prev_measurement.size_bytes == meta.len()
            {
                MediaMeasurement::Reuse(prev_measurement.media_measurement_id)
            } else {
                let task_path = path.clone();
                let task_tool = ffprobe_tool.clone();
                let task_progress = progress.clone();
                let task_in_flight = in_flight.clone();
                let task = tokio::spawn(async move {
                    let _permit = task_in_flight
                        .acquire_owned()
                        .await
                        .expect("we never close this semaphore");
                    let result = probe_file(&task_tool, &task_path).await;
                    task_progress.file_probed();
                    result
                });
                MediaMeasurement::New(task)
            };

            probe_tasks.push(FileMeasurements {
                rel_path,
                meta,
                media_measurement,
            });
        }

        walk_task.await??;

        for FileMeasurements {
            rel_path,
            meta,
            media_measurement,
        } in probe_tasks
        {
            let mut file = FileMeasurement {
                size_bytes: meta.len(),
                mtime: meta.mtime(),
                ctime: Some(meta.ctime()),
                device: Some(meta.dev()),
                inode: Some(meta.ino()),
                hardlink_count: Some(meta.nlink()),
                is_symlink: meta.file_type().is_symlink(),
                media_measurement_id: None,
            };

            let (mime, media, ffprobe_json) = match media_measurement {
                MediaMeasurement::Reuse(reuse_id) => {
                    file.media_measurement_id = reuse_id;
                    (None, None, None)
                }
                MediaMeasurement::New(task) => {
                    // A panic in the task is ours to fix, so it still fails
                    // the ingest. A file that couldn't be probed is noted, and
                    // left out of the ingest.
                    let probed = match task.await? {
                        Ok(probed) => probed,
                        Err(e) => {
                            sink.file_failure(
                                &rel_path,
                                IngestFailureStage::Probe,
                                &format!("{e:#}"),
                            )
                            .await
                            .map_err(CoreError::into_error)?;
                            continue;
                        }
                    };
                    let FileProbeResult {
                        mime,
                        ffprobe,
                        disc_image,
                    } = probed;

                    if let Some(ffprobe) = ffprobe {
                        let mut media = FileMediaMeasurements::from_ffprobe(&ffprobe.output);
                        media.disc_image = disc_image;
                        let ffprobe_json = String::from_utf8(ffprobe.raw_stdout).ok();
                        if ffprobe_json.is_none() {
                            error!("ffprobe output was not valid UTF-8");
                        }
                        (mime, Some(media), ffprobe_json)
                    } else {
                        (mime, None, None)
                    }
                }
            };

            let probed = Probed {
                mime: mime.as_deref(),
                media: media.as_ref(),
                ffprobe_json: ffprobe_json.as_deref(),
            };
            sink.file(&rel_path, &file, probed)
                .await
                .map_err(CoreError::into_error)?;
        }

        Ok(())
    }
}

/// A path under the root, relative to it.
fn rel_path(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root)
        .expect("Descendants of root path must have root as path prefix")
        .to_owned()
}

struct FileMeasurements {
    rel_path: PathBuf,
    meta: std::fs::Metadata,
    media_measurement: MediaMeasurement,
}

enum MediaMeasurement {
    Reuse(Option<FileMediaMeasurementRowId>),
    New(JoinHandle<anyhow::Result<FileProbeResult>>),
}

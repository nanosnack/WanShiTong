use std::path::Path;

use crate::domain::ingest::{FileMeasurement, IngestFailureStage, IngestSink, Probed};
use crate::domain::{
    DataSourceRowId, FileMeasurementRowId, FileMediaMeasurementRowId, IngestRowId,
};
use crate::{CoreContext, CoreError, CoreErrorContext};

/// How many files are written in one transaction. Every commit waits for the
/// disk, so committing each file made recording a library of thousands of
/// files take tens of seconds.
const BATCH_SIZE: usize = 1000;

/// Ingest sink that persists measurements into the database.
///
/// Measurements are written in batches, so they are only in the database once
/// [`IngestSink::flush`] returns. A batch is built in memory and written in a
/// transaction of its own, rather than holding one open across files, because
/// the database has a single writer and probing a file can take a while.
///
/// WARNING: It's critical that the ingest and data source referenced here are
/// not deleted during the ingest task.
///
pub struct DbIngestSink<'a> {
    ctx: &'a CoreContext,
    ingest_id: IngestRowId,
    data_source_id: DataSourceRowId,
    pending: Vec<PendingFile>,
    pending_failures: Vec<PendingFailure>,
}

struct PendingFile {
    rel_path: String,
    file: FileMeasurement,
    media: Option<PendingMedia>,
}

struct PendingFailure {
    rel_path: String,
    stage: IngestFailureStage,
    error: String,
}

struct PendingMedia {
    mime: Option<String>,
    blob: String,
    compressed_ffprobe_json: Option<Vec<u8>>,
}

impl<'a> DbIngestSink<'a> {
    pub fn new(
        ctx: &'a CoreContext,
        ingest_id: IngestRowId,
        data_source_id: DataSourceRowId,
    ) -> Self {
        Self {
            ctx,
            ingest_id,
            data_source_id,
            pending: Vec::new(),
            pending_failures: Vec::new(),
        }
    }

    async fn write_pending(&mut self) -> Result<(), CoreError> {
        if self.pending.is_empty() && self.pending_failures.is_empty() {
            return Ok(());
        }

        let mut txn = self
            .ctx
            .writer
            .begin()
            .await
            .map_err(CoreErrorContext::from)?;

        for PendingFile {
            rel_path,
            file,
            media,
        } in &self.pending
        {
            let FileMeasurement {
                device,
                inode,
                size_bytes,
                mtime,
                ctime,
                hardlink_count,
                is_symlink,
                media_measurement_id,
                ..
            } = file;
            let size = *size_bytes as i64;
            let inode = inode.map(|x| x as i64);
            let device = device.map(|x| x as i64);
            let hardlink_count = hardlink_count.map(|x| x as i64);

            // Insert file idempotently.
            // If the (ingest_id, data_source_id, rel_path) already exists, this is a no-op.
            let file_id: FileMeasurementRowId = sqlx::query_scalar!(
                r#"
            INSERT OR IGNORE INTO file_measurements (
                ingest_id,
                data_source_id,
                rel_path,
                device,
                inode,
                size,
                mtime,
                ctime,
                hardlink_count,
                is_symlink,
                media_measurement_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING id AS "id!: _"
            "#,
                self.ingest_id,
                self.data_source_id,
                rel_path,
                device,
                inode,
                size,
                mtime,
                ctime,
                hardlink_count,
                is_symlink,
                media_measurement_id
            )
            .fetch_one(&mut *txn)
            .await
            .map_err(CoreErrorContext::from)?;

            if let Some(PendingMedia {
                mime,
                blob,
                compressed_ffprobe_json,
            }) = media
            {
                let media_id: FileMediaMeasurementRowId = sqlx::query_scalar!(
                    r#"
                INSERT INTO file_media_measurements (
                    file_measurement_id, mime_type, data, ffprobe_json
                )
                VALUES (?, ?, ?, ?)
                RETURNING id as "id!: _"
                "#,
                    file_id,
                    mime,
                    blob,
                    compressed_ffprobe_json,
                )
                .fetch_one(&mut *txn)
                .await
                .map_err(CoreErrorContext::from)?;

                // TODO: can we think of something more elegant than closing the
                // referential cycle with an UPDATE?
                sqlx::query!(
                    "
                UPDATE file_measurements
                SET media_measurement_id = ?
                WHERE id = ?",
                    media_id,
                    file_id
                )
                .execute(&mut *txn)
                .await
                .map_err(CoreErrorContext::from)?;
            }
        }

        for PendingFailure {
            rel_path,
            stage,
            error,
        } in &self.pending_failures
        {
            sqlx::query!(
                "INSERT INTO ingest_file_failures (ingest_id, rel_path, stage, error)
                 VALUES (?, ?, ?, ?)",
                self.ingest_id,
                rel_path,
                stage,
                error,
            )
            .execute(&mut *txn)
            .await
            .map_err(CoreErrorContext::from)?;
        }

        txn.commit().await.map_err(CoreErrorContext::from)?;
        self.pending.clear();
        self.pending_failures.clear();

        Ok(())
    }
}

#[async_trait::async_trait]
impl<'a> IngestSink for DbIngestSink<'a> {
    async fn file(
        &mut self,
        rel_path: &Path,
        file: &FileMeasurement,
        probed: Probed<'_>,
    ) -> Result<(), CoreError> {
        let Probed {
            mime,
            media,
            ffprobe_json,
        } = probed;
        let media = media.map(|media| PendingMedia {
            mime: mime.map(str::to_owned),
            blob: crate::json::encode(media),
            // TODO: put compression on a compute pool
            compressed_ffprobe_json: ffprobe_json
                .map(|str| zstd::bulk::compress(str.as_bytes(), 0).unwrap()),
        });
        self.pending.push(PendingFile {
            rel_path: rel_path.to_string_lossy().into_owned(),
            file: file.clone(),
            media,
        });

        if self.pending.len() >= BATCH_SIZE {
            self.write_pending().await?;
        }
        Ok(())
    }

    async fn file_failure(
        &mut self,
        rel_path: &Path,
        stage: IngestFailureStage,
        error: &str,
    ) -> Result<(), CoreError> {
        self.pending_failures.push(PendingFailure {
            rel_path: rel_path.to_string_lossy().into_owned(),
            stage,
            error: error.to_owned(),
        });
        if self.pending.len() + self.pending_failures.len() >= BATCH_SIZE {
            self.write_pending().await?;
        }
        Ok(())
    }

    async fn flush(&mut self) -> Result<(), CoreError> {
        self.write_pending().await
    }
}

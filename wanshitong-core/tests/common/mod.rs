//! Fixtures shared by the integration tests.
//!
//! Each test binary compiles this module and uses only some of it.
#![allow(dead_code)]

use wanshitong_core::CoreContext;
use wanshitong_core::commands::{create_data_source, finish_ingest, start_ingest};
use wanshitong_core::domain::Probed;
use wanshitong_core::domain::{
    DataSourceConfig, DataSourceUuid, DbIngestSink, FileMeasurement, FileMeasurementRowId,
    FileSystemSource, IngestSink, IngestStats, IngestUuid, InterpretationRowId,
};
use wanshitong_core::queries::list_facts;

/// Ingest `rel_paths` as plain files into a new data source.
pub async fn ingest_files(
    ctx: &CoreContext,
    name: &str,
    rel_paths: &[&str],
) -> (DataSourceUuid, IngestUuid) {
    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: "/media".into(),
        follow_symlinks: false,
    });
    let source = create_data_source(ctx, name, config).await.unwrap();
    let ingest = start_ingest(ctx, source.uuid).await.unwrap();
    let mut sink = DbIngestSink::new(ctx, ingest.row_id, ingest.data_source.row_id);
    for rel_path in rel_paths {
        let file = FileMeasurement {
            device: None,
            inode: None,
            size_bytes: 0,
            mtime: 0,
            ctime: None,
            hardlink_count: None,
            is_symlink: false,
            media_measurement_id: None,
        };
        sink.file(rel_path.as_ref(), &file, Probed::default())
            .await
            .unwrap();
    }
    let stats = IngestStats {
        files_seen: rel_paths.len() as u64,
        files_probed: 0,
        files_recorded: rel_paths.len() as u64,
        media_files_recorded: 0,
        files_failed: 0,
    };
    sink.flush().await.unwrap();
    finish_ingest(ctx, ingest.row_id, true, stats)
        .await
        .unwrap();
    (source.uuid, ingest.uuid)
}

pub async fn file_id(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
    rel_path: &str,
) -> FileMeasurementRowId {
    list_facts(ctx, interpretation_id)
        .await
        .unwrap()
        .files
        .iter()
        .find(|(path, _)| path.rel_path == std::path::Path::new(rel_path))
        .unwrap()
        .1
        .row_id
}

/// Asserts in a changeset of its own, and accepts it, as a correction in the
/// web UI does.
pub async fn accept_assertion(
    ctx: &CoreContext,
    assertion: wanshitong_core::commands::NewAssertion,
) -> wanshitong_core::domain::AssertionUuid {
    use wanshitong_core::commands::{
        NewChangeset, accept_changeset, create_assertion, create_changeset,
    };
    let changeset = create_changeset(
        ctx,
        NewChangeset {
            author: "user".into(),
            description: "A correction".into(),
        },
    )
    .await
    .unwrap();
    let uuid = create_assertion(ctx, changeset, assertion).await.unwrap();
    accept_changeset(ctx, changeset).await.unwrap();
    uuid
}

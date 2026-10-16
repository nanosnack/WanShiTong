//! The config file's probing settings reach ingests, through the database's
//! config snapshot.

use tempfile::tempdir;
use wanshitong_app::commands::{IngestStart, apply_config, create_data_source, start_ingest};
use wanshitong_app::domain::{DataSourceConfig, FileSystemSource, IngestFailureStage};
use wanshitong_app::queries::{list_ingest_file_failures, wait_for_ingest};
use wanshitong_app::{AppContext, Config, CoreContext};

#[tokio::test]
async fn ingests_probe_with_the_configured_ffprobe() {
    let tmp = tempdir().unwrap();
    std::fs::write(tmp.path().join("example.mkv"), "not a video").unwrap();
    let config = Config {
        ffprobe_path: tmp.path().join("no-such-ffprobe").display().to_string(),
        ..Default::default()
    };
    let core = CoreContext::new_in_memory().await.unwrap();
    let ctx = AppContext::new(core, config, "/fake/data");
    apply_config(&ctx).await.unwrap();

    let source = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: tmp.path().into(),
        follow_symlinks: false,
    });
    let source = create_data_source(&ctx, "foo", source).await.unwrap();
    let IngestStart::Started(ingest_id) = start_ingest(&ctx, source.uuid).await.unwrap() else {
        panic!("no ingest of it was running");
    };
    let snapshot = wait_for_ingest(&ctx, ingest_id).await.unwrap();
    assert_eq!(snapshot.error, None);
    assert_eq!(snapshot.files_failed, 1);

    let failures = list_ingest_file_failures(&ctx, ingest_id).await.unwrap();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].stage, IngestFailureStage::Probe);
    assert!(
        failures[0].error.contains("no-such-ffprobe"),
        "{}",
        failures[0].error
    );
}

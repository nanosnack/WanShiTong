mod common;

use common::{wait_for_finished_ingests, wait_for_interpretations};
use std::time::Duration;
use tempfile::tempdir;
use wanshitong_app::commands::{create_data_source, watch_data_sources};
use wanshitong_app::domain::{DataSourceConfig, FileSystemSource};
use wanshitong_app::queries::{list_files, list_ingests};
use wanshitong_app::{AppContext, Config, CoreContext};
use wanshitong_core::queries::FileSearchQuery;

#[tokio::test]
async fn a_data_source_is_ingested_once_its_files_change() {
    let tmp = tempdir().unwrap();
    tokio::fs::write(tmp.path().join("old.txt"), "old")
        .await
        .unwrap();
    let config = Config {
        watch_quiet_period_secs: 1,
        ..Default::default()
    };
    let ctx = AppContext::new(CoreContext::new_in_memory().await.unwrap(), config, "/fake");
    let source = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: tmp.path().into(),
        follow_symlinks: false,
    });
    create_data_source(&ctx, "Movies", source).await.unwrap();
    watch_data_sources(&ctx).await.unwrap();

    tokio::fs::write(tmp.path().join("new.txt"), "new")
        .await
        .unwrap();

    let ingests = wait_for_finished_ingests(&ctx, 1).await;
    assert_eq!(ingests[0].files_recorded(), 2);

    // That ingest read both files, which isn't a change, so it doesn't lead
    // to another.
    wait_for_interpretations(&ctx).await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(list_ingests(&ctx).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_data_source_created_while_watching_is_watched() {
    let tmp = tempdir().unwrap();
    let config = Config {
        watch_quiet_period_secs: 1,
        ..Default::default()
    };
    let ctx = AppContext::new(CoreContext::new_in_memory().await.unwrap(), config, "/fake");
    watch_data_sources(&ctx).await.unwrap();
    let source = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: tmp.path().into(),
        follow_symlinks: false,
    });
    create_data_source(&ctx, "Movies", source).await.unwrap();

    // The watcher starts watching it in its own time, so a change made before
    // then goes unseen, and is made again.
    let path = tmp.path().join("new.txt");
    for _ in 0..5 {
        tokio::fs::write(&path, "new").await.unwrap();
        tokio::time::sleep(Duration::from_millis(2500)).await;
        if !list_ingests(&ctx).await.unwrap().is_empty() {
            break;
        }
    }

    let ingests = wait_for_finished_ingests(&ctx, 1).await;
    assert_eq!(ingests[0].files_recorded(), 1);
    wait_for_interpretations(&ctx).await;
}

#[tokio::test]
async fn a_file_being_written_holds_back_its_ingest_until_closed() {
    use std::io::Write;

    let tmp = tempdir().unwrap();
    let config = Config {
        watch_quiet_period_secs: 1,
        ..Default::default()
    };
    let ctx = AppContext::new(CoreContext::new_in_memory().await.unwrap(), config, "/fake");
    let source = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: tmp.path().into(),
        follow_symlinks: false,
    });
    create_data_source(&ctx, "Movies", source).await.unwrap();
    watch_data_sources(&ctx).await.unwrap();

    // A copy that pauses for longer than the quiet period.
    let mut file = std::fs::File::create(tmp.path().join("movie.mkv")).unwrap();
    file.write_all(&[0; 1000]).unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(list_ingests(&ctx).await.unwrap().is_empty());
    file.write_all(&[0; 1000]).unwrap();
    drop(file);

    wait_for_finished_ingests(&ctx, 1).await;
    wait_for_interpretations(&ctx).await;
    let every_file = FileSearchQuery {
        identified: true,
        non_videos: true,
        ..Default::default()
    };
    let files = list_files(&ctx, &every_file).await.unwrap().files;
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].size, Some(2000));
}

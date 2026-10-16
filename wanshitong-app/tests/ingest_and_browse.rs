mod common;

use common::{wait_for_finished_ingests, wait_for_interpretations};
use tempfile::tempdir;
use wanshitong_app::commands::{IngestStart, create_data_source, start_ingest};
use wanshitong_app::domain::{DataSourceConfig, FileSystemSource};
use wanshitong_app::ingest::{IngestEvent, IngestProgressSnapshot, IngestStatus};
use wanshitong_app::queries::{list_data_sources, list_files, list_ingests, wait_for_ingest};
use wanshitong_app::{AppContext, CoreContext, CoreErrorContext};
use wanshitong_core::domain::{FileSummary, Ingest, IngestDataSource, IngestStats};
use wanshitong_core::queries::{FileSearchQuery, FileSummaryList};

#[tokio::test]
async fn create_and_query_data_sources() {
    let core = CoreContext::new_in_memory().await.unwrap();
    let ctx = AppContext::new(core, Default::default(), "/fake/data");

    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: "/foo/bar".into(),
        follow_symlinks: false,
    });

    let source1 = create_data_source(&ctx, "foo", config.clone())
        .await
        .unwrap();
    let data_sources = list_data_sources(&ctx).await.unwrap();
    assert_eq!(data_sources, vec![source1.clone()]);

    // Duplicate root paths are allowed! They might have other different
    // configuration.
    let source2 = create_data_source(&ctx, "bar", config.clone())
        .await
        .unwrap();
    let data_sources = list_data_sources(&ctx).await.unwrap();
    assert_eq!(data_sources, vec![source1.clone(), source2]);

    // Duplicate name will fail.
    let err = create_data_source(&ctx, "foo", config).await.unwrap_err();
    assert!(matches!(
        err.core_context(),
        Some(CoreErrorContext::DataSourceNameExists(_))
    ));
}

#[tokio::test]
async fn ingest_then_query_new_files_and_measurements() {
    let tmp = tempdir().unwrap();
    let file_name = "example.txt";
    let file_content = "hello world";
    let file_path = tmp.path().join(file_name);
    tokio::fs::write(file_path, file_content).await.unwrap();

    let core = CoreContext::new_in_memory().await.unwrap();
    let ctx = AppContext::new(core, Default::default(), "/fake/data");

    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: tmp.path().into(),
        follow_symlinks: false,
    });
    let source = create_data_source(&ctx, "foo", config).await.unwrap();

    let IngestStart::Started(ingest_uuid) = start_ingest(&ctx, source.uuid).await.unwrap() else {
        panic!("no ingest of it was running");
    };
    let IngestProgressSnapshot {
        files_seen,
        files_probed,
        media_files_recorded,
        files_recorded,
        files_failed,
        started_at,
        stopped_at,
        logged_events,
        error,
    } = wait_for_ingest(&ctx, ingest_uuid).await.unwrap();
    let stopped_at = stopped_at.unwrap();
    assert!(error.is_none());
    assert_eq!(logged_events, vec![]);
    assert_eq!(files_seen, 1);
    assert_eq!(files_probed, 1);
    assert_eq!(media_files_recorded, 0);
    assert_eq!(files_recorded, 1);
    assert_eq!(files_failed, 0);
    assert!(started_at < stopped_at);

    // The ingest was marked complete in the database.
    let ingests = list_ingests(&ctx).await.unwrap();
    assert_eq!(ingests.len(), 1);
    let ingest = ingests.into_iter().next().unwrap();
    let IngestStatus {
        ingest:
            Ingest {
                id: _,
                uuid,
                started_at,
                stats:
                    IngestStats {
                        files_seen: persisted_files_seen,
                        files_probed: persisted_files_probed,
                        files_recorded: persisted_files_recorded,
                        media_files_recorded: persisted_media_files_recorded,
                        files_failed: persisted_files_failed,
                    },
                stopped_at,
                success,
            },
        data_source: ingest_source,
        progress: _,
    } = ingest;
    let stopped_at = stopped_at.unwrap();
    assert_eq!(uuid, ingest_uuid);
    assert!(success);
    // NOTE: These timestamps are persisted in seconds, not nanoseconds. They
    // might be equal.
    assert!(started_at <= stopped_at);
    assert_eq!(persisted_files_seen, 1);
    assert_eq!(persisted_files_probed, 1);
    assert_eq!(persisted_files_recorded, 1);
    assert_eq!(persisted_media_files_recorded, 0);
    assert_eq!(persisted_files_failed, 0);
    assert_eq!(
        ingest_source,
        IngestDataSource {
            uuid: source.uuid,
            name: source.name.clone(),
            created_at: source.created_at,
            config: source.config.clone(),
        }
    );

    wait_for_interpretations(&ctx).await;
    let FileSummaryList { files, next_cursor } = list_files(&ctx, &every_file()).await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(next_cursor, None);
    let FileSummary {
        data_source_uuid,
        data_source_name,
        rel_path,
        size,
        disc: _,
    } = &files[0];
    assert_eq!(*data_source_uuid, source.uuid);
    assert_eq!(*data_source_name, source.name);
    assert_eq!(rel_path, file_name);
    assert_eq!(size.unwrap(), file_content.len() as i64);

    // Do another identical ingest; should not add duplicate files, only new
    // measurements.
    let IngestStart::Started(ingest_id) = start_ingest(&ctx, source.uuid).await.unwrap() else {
        panic!("no ingest of it was running");
    };
    let IngestProgressSnapshot {
        files_seen,
        files_probed,
        media_files_recorded,
        files_recorded,
        files_failed,
        started_at,
        stopped_at,
        logged_events,
        error,
    } = wait_for_ingest(&ctx, ingest_id).await.unwrap();
    let stopped_at = stopped_at.unwrap();
    assert!(error.is_none());
    assert_eq!(logged_events, vec![]);
    assert_eq!(files_seen, 1);
    assert_eq!(files_probed, 0); // reuses previous probe
    assert_eq!(media_files_recorded, 0);
    assert_eq!(files_recorded, 1);
    assert_eq!(files_failed, 0);
    assert!(started_at < stopped_at);

    wait_for_interpretations(&ctx).await;
    let FileSummaryList { files, next_cursor } = list_files(&ctx, &every_file()).await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(next_cursor, None);
    let FileSummary {
        data_source_uuid,
        data_source_name,
        rel_path,
        size,
        disc: _,
    } = &files[0];
    assert_eq!(*data_source_uuid, source.uuid);
    assert_eq!(*data_source_name, source.name);
    assert_eq!(rel_path, file_name);
    assert_eq!(size.unwrap(), file_content.len() as i64);

    // Replace the file, keeping its mtime (as `cp -p` would). The new size
    // gives the change away, so the file is probed again.
    let file_path = tmp.path().join(file_name);
    let mtime = std::fs::metadata(&file_path).unwrap().modified().unwrap();
    std::fs::write(&file_path, "hello again, world").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&file_path)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    let IngestStart::Started(ingest_id) = start_ingest(&ctx, source.uuid).await.unwrap() else {
        panic!("no ingest of it was running");
    };
    let IngestProgressSnapshot {
        files_probed,
        error,
        ..
    } = wait_for_ingest(&ctx, ingest_id).await.unwrap();
    assert!(error.is_none());
    assert_eq!(files_probed, 1);
}

#[tokio::test]
async fn ingest_failure_when_root_path_missing() {
    let tmp = tempdir().unwrap();

    let core = CoreContext::new_in_memory().await.unwrap();
    let ctx = AppContext::new(core, Default::default(), "/fake/data");

    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: tmp.path().join("does/not/exist"),
        follow_symlinks: false,
    });
    let source = create_data_source(&ctx, "foo", config).await.unwrap();

    let IngestStart::Started(ingest_id) = start_ingest(&ctx, source.uuid).await.unwrap() else {
        panic!("no ingest of it was running");
    };
    let IngestProgressSnapshot {
        files_seen,
        files_probed,
        media_files_recorded,
        files_recorded,
        files_failed,
        started_at,
        stopped_at,
        logged_events,
        error,
    } = wait_for_ingest(&ctx, ingest_id).await.unwrap();
    let stopped_at = stopped_at.unwrap();
    assert!(error.is_some());
    assert!(matches!(
        &logged_events[..],
        [IngestEvent {
            name,
            time: _,
            payload
        }] if name == "Fatal Error" && payload.contains("No such file")
    ));
    assert_eq!(files_seen, 0);
    assert_eq!(files_probed, 0);
    assert_eq!(media_files_recorded, 0);
    assert_eq!(files_recorded, 0);
    assert_eq!(files_failed, 0);
    assert!(started_at < stopped_at);

    // The ingest was marked failed in the database.
    let ingests = list_ingests(&ctx).await.unwrap();
    assert_eq!(ingests.len(), 1);
    let ingest = ingests.into_iter().next().unwrap();
    let IngestStatus {
        ingest:
            Ingest {
                id: _,
                uuid,
                started_at,
                stats:
                    IngestStats {
                        files_seen,
                        files_probed,
                        files_recorded,
                        media_files_recorded,
                        files_failed,
                    },
                stopped_at,
                success,
            },
        data_source: ingest_source,
        progress: _,
    } = ingest;
    let stopped_at = stopped_at.unwrap();
    assert_eq!(uuid, ingest_id);
    assert!(!success);
    // NOTE: These timestamps are persisted in seconds, not nanoseconds. They
    // might be equal.
    assert!(started_at <= stopped_at);
    assert_eq!(files_seen, 0);
    assert_eq!(files_probed, 0);
    assert_eq!(files_recorded, 0);
    assert_eq!(media_files_recorded, 0);
    assert_eq!(files_failed, 0);
    assert_eq!(
        ingest_source,
        IngestDataSource {
            uuid: source.uuid,
            name: source.name.clone(),
            created_at: source.created_at,
            config: source.config.clone(),
        }
    );
}

#[tokio::test]
async fn list_files_pages_through_every_file_once() {
    let tmp = tempdir().unwrap();
    let mut expected = Vec::new();
    for dir in ["a", "b"] {
        tokio::fs::create_dir(tmp.path().join(dir)).await.unwrap();
        for i in 0..4 {
            let rel_path = format!("{dir}/{i}.txt");
            tokio::fs::write(tmp.path().join(&rel_path), "x")
                .await
                .unwrap();
            expected.push(rel_path);
        }
    }

    let core = CoreContext::new_in_memory().await.unwrap();
    let ctx = AppContext::new(core, Default::default(), "/fake/data");
    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: tmp.path().into(),
        follow_symlinks: false,
    });
    let source = create_data_source(&ctx, "foo", config).await.unwrap();
    let IngestStart::Started(ingest_id) = start_ingest(&ctx, source.uuid).await.unwrap() else {
        panic!("no ingest of it was running");
    };
    wait_for_ingest(&ctx, ingest_id).await.unwrap();
    wait_for_interpretations(&ctx).await;

    // Pages smaller than the data source, so each cursor points inside it.
    let mut seen = Vec::new();
    let mut query = FileSearchQuery {
        identified: true,
        non_videos: true,
        limit: 3,
        ..Default::default()
    };
    loop {
        let FileSummaryList { files, next_cursor } = list_files(&ctx, &query).await.unwrap();
        seen.extend(files.into_iter().map(|f| f.rel_path));
        match next_cursor {
            Some(cursor) => query.cursor = Some(cursor),
            None => break,
        }
        assert!(seen.len() <= expected.len(), "paging did not end: {seen:?}");
    }
    assert_eq!(seen, expected);
}

/// Starting an ingest used to fail with "database is locked" while another
/// ingest was committing its files, on a file database (in-memory ones have a
/// single connection, so they never raced). Ingests of one data source don't
/// overlap, so these are of three, with the same files.
#[tokio::test(flavor = "multi_thread")]
async fn start_ingest_while_another_is_running() {
    let src = tempdir().unwrap();
    for i in 0..1000 {
        std::fs::write(src.path().join(format!("{i}.txt")), "x").unwrap();
    }
    let data = tempdir().unwrap();
    let core = CoreContext::new_in_file(data.path()).await.unwrap();
    let ctx = AppContext::new(core, Default::default(), data.path());

    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: src.path().into(),
        follow_symlinks: false,
    });
    let mut ingest_ids = Vec::new();
    for i in 0..3 {
        let source = create_data_source(&ctx, format!("foo{i}"), config.clone())
            .await
            .unwrap();
        let IngestStart::Started(ingest_id) = start_ingest(&ctx, source.uuid).await.unwrap() else {
            panic!("no ingest of it was running");
        };
        ingest_ids.push(ingest_id);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    for ingest_id in ingest_ids {
        let snapshot = wait_for_ingest(&ctx, ingest_id).await.unwrap();
        assert_eq!(snapshot.error, None);
    }
}

/// A search for every file, rather than only the unidentified videos.
fn every_file() -> FileSearchQuery {
    FileSearchQuery {
        identified: true,
        non_videos: true,
        ..Default::default()
    }
}

#[tokio::test]
async fn requests_while_an_ingest_runs_queue_one_more() {
    let tmp = tempdir().unwrap();
    tokio::fs::write(tmp.path().join("example.txt"), "hello")
        .await
        .unwrap();
    let core = CoreContext::new_in_memory().await.unwrap();
    let ctx = AppContext::new(core, Default::default(), "/fake/data");
    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: tmp.path().into(),
        follow_symlinks: false,
    });
    let source = create_data_source(&ctx, "foo", config).await.unwrap();

    let first = start_ingest(&ctx, source.uuid).await.unwrap();
    assert!(matches!(first, IngestStart::Started(_)));
    for _ in 0..3 {
        let next = start_ingest(&ctx, source.uuid).await.unwrap();
        assert_eq!(next, IngestStart::Queued);
    }

    // The queued requests make one ingest, which runs after the first.
    let ingests = wait_for_finished_ingests(&ctx, 2).await;
    assert!(ingests[0].stopped_at().unwrap() <= ingests[1].started_at());
    wait_for_interpretations(&ctx).await;

    // Then the data source can be ingested again.
    let last = start_ingest(&ctx, source.uuid).await.unwrap();
    assert!(matches!(last, IngestStart::Started(_)));
    wait_for_finished_ingests(&ctx, 3).await;
    wait_for_interpretations(&ctx).await;
}

/// Restores a path's permissions when dropped, so the temporary directory can
/// be removed however the test ends.
struct Reopen(std::path::PathBuf, u32);

impl Drop for Reopen {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(self.1));
    }
}

/// One file's trouble doesn't fail the ingest: the file is noted as failed and
/// left out, and the rest are recorded.
#[tokio::test]
async fn unreadable_files_are_noted_and_skipped() {
    use std::os::unix::fs::PermissionsExt;
    use wanshitong_app::domain::IngestFailureStage;
    use wanshitong_app::queries::list_ingest_file_failures;

    let tmp = tempdir().unwrap();
    tokio::fs::write(tmp.path().join("ok.txt"), "fine")
        .await
        .unwrap();
    let secret = tmp.path().join("secret.txt");
    tokio::fs::write(&secret, "no peeking").await.unwrap();
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o000)).unwrap();
    let _reopen_secret = Reopen(secret.clone(), 0o644);
    let closed = tmp.path().join("closed");
    std::fs::create_dir(&closed).unwrap();
    tokio::fs::write(closed.join("inside.txt"), "hidden")
        .await
        .unwrap();
    std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o000)).unwrap();
    let _reopen_closed = Reopen(closed.clone(), 0o755);
    if std::fs::read(&secret).is_ok() {
        // Permissions don't stop whoever is running the tests, e.g. root.
        return;
    }

    let core = CoreContext::new_in_memory().await.unwrap();
    let ctx = AppContext::new(core, Default::default(), "/fake/data");
    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: tmp.path().into(),
        follow_symlinks: false,
    });
    let source = create_data_source(&ctx, "foo", config).await.unwrap();
    let IngestStart::Started(ingest_id) = start_ingest(&ctx, source.uuid).await.unwrap() else {
        panic!("no ingest of it was running");
    };
    let snapshot = wait_for_ingest(&ctx, ingest_id).await.unwrap();
    assert_eq!(snapshot.error, None);
    assert_eq!(snapshot.logged_events, vec![]);
    // The closed directory isn't a file, so it's not seen as one.
    assert_eq!(snapshot.files_seen, 2);
    assert_eq!(snapshot.files_recorded, 1);
    assert_eq!(snapshot.files_failed, 2);

    let failures = list_ingest_file_failures(&ctx, ingest_id).await.unwrap();
    let mut noted: Vec<_> = failures
        .iter()
        .map(|f| (f.rel_path.to_string_lossy().into_owned(), f.stage))
        .collect();
    noted.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        noted,
        vec![
            ("closed".to_string(), IngestFailureStage::Walk),
            ("secret.txt".to_string(), IngestFailureStage::Probe),
        ]
    );
    for failure in &failures {
        assert_eq!(failure.data_source, source.uuid);
        assert!(
            failure.error.contains("ermission denied"),
            "{}",
            failure.error
        );
    }

    // The ingest succeeded, and says how many files it's missing.
    let ingests = list_ingests(&ctx).await.unwrap();
    assert_eq!(ingests.len(), 1);
    assert!(ingests[0].ingest.success);
    assert_eq!(ingests[0].ingest.stats.files_failed, 2);
    assert_eq!(ingests[0].ingest.stats.files_recorded, 1);

    wait_for_interpretations(&ctx).await;
    let FileSummaryList { files, .. } = list_files(&ctx, &every_file()).await.unwrap();
    let names: Vec<_> = files.iter().map(|f| f.rel_path.as_str()).collect();
    assert_eq!(names, vec!["ok.txt"]);
}

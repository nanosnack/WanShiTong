mod common;

use common::ingest_files;
use std::collections::BTreeSet;
use wanshitong_core::CoreContext;
use wanshitong_core::commands::create_interpretation;
use wanshitong_core::queries::list_facts;

/// The sink writes files in batches, so an ingest of more files than fit in
/// one must still record each of them exactly once.
#[tokio::test]
async fn ingest_records_every_file_across_batches() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let paths: Vec<String> = (0..2500).map(|i| format!("file{i}.mkv")).collect();
    let rel_paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    let (source, ingest) = ingest_files(&ctx, "Movies", &rel_paths).await;

    let interpretation = create_interpretation(&ctx, "test", &[(source, ingest)])
        .await
        .unwrap()
        .row_id;
    let files = list_facts(&ctx, interpretation).await.unwrap().files;
    let recorded: BTreeSet<_> = files
        .iter()
        .map(|(path, _)| path.rel_path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(files.len(), paths.len());
    assert_eq!(recorded, paths.into_iter().collect());
}

/// An NFO's text is a measurement, listed with the file's facts.
#[tokio::test]
async fn ingest_records_an_nfos_text() {
    use wanshitong_core::commands::{create_data_source, finish_ingest, start_ingest};
    use wanshitong_core::domain::{
        DataSourceConfig, DbIngestSink, FileMeasurement, FileSystemSource, IngestSink, IngestStats,
        Probed,
    };

    let ctx = CoreContext::new_in_memory().await.unwrap();
    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: "/media".into(),
        follow_symlinks: false,
    });
    let source = create_data_source(&ctx, "Movies", config).await.unwrap();
    let ingest = start_ingest(&ctx, source.uuid).await.unwrap();
    let mut sink = DbIngestSink::new(&ctx, ingest.row_id, ingest.data_source.row_id);
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
    let text = "<movie><title>Heat</title></movie>";
    sink.file(
        "Heat (1995)/Heat.nfo".as_ref(),
        &file,
        Probed {
            nfo_text: Some(text),
            ..Probed::default()
        },
    )
    .await
    .unwrap();
    sink.file("Heat (1995)/Heat.mkv".as_ref(), &file, Probed::default())
        .await
        .unwrap();
    sink.flush().await.unwrap();
    let stats = IngestStats {
        files_seen: 2,
        files_probed: 0,
        files_recorded: 2,
        media_files_recorded: 0,
        files_failed: 0,
    };
    finish_ingest(&ctx, ingest.row_id, true, stats)
        .await
        .unwrap();

    let interpretation = create_interpretation(&ctx, "test", &[(source.uuid, ingest.uuid)])
        .await
        .unwrap()
        .row_id;
    let files = list_facts(&ctx, interpretation).await.unwrap().files;
    let nfo_texts: Vec<(String, Option<String>)> = files
        .into_iter()
        .map(|(path, facts)| (path.rel_path.to_string_lossy().into_owned(), facts.nfo_text))
        .collect();
    assert_eq!(
        nfo_texts,
        vec![
            ("Heat (1995)/Heat.mkv".to_string(), None),
            ("Heat (1995)/Heat.nfo".to_string(), Some(text.to_string())),
        ]
    );
}

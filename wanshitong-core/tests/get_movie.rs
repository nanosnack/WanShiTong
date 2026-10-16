use std::path::Path;
use wanshitong_core::CoreContext;
use wanshitong_core::commands::{
    NewCatalog, NewMovie, NewMovieExpression, NewMovieFile, create_data_source,
    create_interpretation, finish_ingest, record_catalog, start_ingest,
};
use wanshitong_core::domain::Probed;
use wanshitong_core::domain::{
    ContainerMeasurement, DataSourceConfig, DbIngestSink, FileMeasurement,
    FileMediaMeasurementRowId, FileMediaMeasurements, FileRole, FileSystemSource, IngestSink,
    IngestStats,
};
use wanshitong_core::queries::{MovieSearchQuery, get_movie, list_facts, list_movies};

fn file(media_measurement_id: Option<FileMediaMeasurementRowId>) -> FileMeasurement {
    FileMeasurement {
        device: None,
        inode: None,
        size_bytes: 0,
        mtime: 0,
        ctime: None,
        hardlink_count: None,
        is_symlink: false,
        media_measurement_id,
    }
}

#[tokio::test]
async fn shows_media_measurements_reused_from_an_earlier_ingest() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: "/media".into(),
        follow_symlinks: false,
    });
    let source = create_data_source(&ctx, "media", config).await.unwrap();
    let stats = IngestStats {
        files_seen: 1,
        files_probed: 0,
        files_recorded: 1,
        media_files_recorded: 1,
        files_failed: 0,
    };

    // The first ingest probes the file.
    let media = FileMediaMeasurements {
        container: Some(ContainerMeasurement {
            format: "matroska,webm".into(),
            duration_ms: Some(10_236_000),
            bitrate: None,
            stream_count: 0,
            tags: None,
        }),
        video: vec![],
        audio: vec![],
        text: vec![],
        unclassified: vec![],
        disc_image: None,
    };
    let first = start_ingest(&ctx, source.uuid).await.unwrap();
    let mut sink = DbIngestSink::new(&ctx, first.row_id, first.data_source.row_id);
    sink.file(
        Path::new("heat.mkv"),
        &file(None),
        Probed {
            mime: Some("video/x-matroska"),
            media: Some(&media),
            ffprobe_json: None,
        },
    )
    .await
    .unwrap();
    sink.flush().await.unwrap();
    finish_ingest(&ctx, first.row_id, true, stats)
        .await
        .unwrap();

    // The second finds the file unchanged, so it reuses the first's media
    // measurement instead of probing again.
    let second = start_ingest(&ctx, source.uuid).await.unwrap();
    let reused =
        second.data_source.prev_media_measurements[Path::new("heat.mkv")].media_measurement_id;
    assert!(reused.is_some());
    let mut sink = DbIngestSink::new(&ctx, second.row_id, second.data_source.row_id);
    sink.file(Path::new("heat.mkv"), &file(reused), Probed::default())
        .await
        .unwrap();
    sink.flush().await.unwrap();
    finish_ingest(&ctx, second.row_id, true, stats)
        .await
        .unwrap();

    let interpretation = create_interpretation(&ctx, "test", &[(source.uuid, second.uuid)])
        .await
        .unwrap()
        .row_id;
    let file_id = list_facts(&ctx, interpretation).await.unwrap().files[0]
        .1
        .row_id;
    record_catalog(
        &ctx,
        interpretation,
        NewCatalog {
            movies: vec![NewMovie {
                title: "Heat".into(),
                expressions: vec![NewMovieExpression { edition: None }],
                tmdb_id: Some(949),
                files: vec![NewMovieFile {
                    file_id,
                    role: Some(FileRole::Main),
                    expression: Some(0),
                    manifestation: Some(0),
                    part: None,
                }],
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let movies = list_movies(&ctx, interpretation, &MovieSearchQuery::default())
        .await
        .unwrap()
        .movies;
    let movie = get_movie(&ctx, movies[0].uuid).await.unwrap().unwrap();
    let duration_ms = movie
        .media
        .and_then(|media| media.container)
        .and_then(|container| container.duration_ms);
    assert_eq!(duration_ms, Some(10_236_000));
}

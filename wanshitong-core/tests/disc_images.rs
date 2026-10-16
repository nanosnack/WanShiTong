use std::path::Path;
use wanshitong_core::CoreContext;
use wanshitong_core::commands::{
    create_data_source, create_interpretation, finish_ingest, start_ingest,
};
use wanshitong_core::domain::Probed;
use wanshitong_core::domain::{
    ContainerMeasurement, DataSourceConfig, DbIngestSink, DiscKind, FileMeasurement,
    FileMediaMeasurements, FileSystemSource, IngestSink, IngestStats,
};
use wanshitong_core::queries::{FileTreeQuery, list_facts, list_file_tree};

#[tokio::test]
async fn a_disc_image_is_a_disc_of_its_own() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: "/media".into(),
        follow_symlinks: false,
    });
    let source = create_data_source(&ctx, "media", config).await.unwrap();
    let image = "AnimeMovies/The.Wind.Rises.2013.Blu-ray.1080p.AVC.LPCM.2.0-Qux.iso";
    // Its longest title's, as ffprobe reads it through libbluray.
    let media = FileMediaMeasurements {
        container: Some(ContainerMeasurement {
            format: "mpegts".into(),
            duration_ms: Some(7_591_196),
            bitrate: None,
            stream_count: 4,
            tags: None,
        }),
        video: vec![],
        audio: vec![],
        text: vec![],
        unclassified: vec![],
        disc_image: Some(DiscKind::BluRay),
    };
    let file = FileMeasurement {
        device: None,
        inode: None,
        size_bytes: 50_008_686_592,
        mtime: 0,
        ctime: None,
        hardlink_count: None,
        is_symlink: false,
        media_measurement_id: None,
    };
    let ingest = start_ingest(&ctx, source.uuid).await.unwrap();
    let mut sink = DbIngestSink::new(&ctx, ingest.row_id, ingest.data_source.row_id);
    sink.file(
        Path::new(image),
        &file,
        Probed {
            mime: Some("application/octet-stream"),
            media: Some(&media),
            ffprobe_json: None,
        },
    )
    .await
    .unwrap();
    let stats = IngestStats {
        files_seen: 1,
        files_probed: 1,
        files_recorded: 1,
        media_files_recorded: 1,
        files_failed: 0,
    };
    sink.flush().await.unwrap();
    finish_ingest(&ctx, ingest.row_id, true, stats)
        .await
        .unwrap();
    let interpretation = create_interpretation(&ctx, "test", &[(source.uuid, ingest.uuid)])
        .await
        .unwrap()
        .row_id;

    // It's a Blu-ray, which is the image, playing as long as its longest title.
    let facts = list_facts(&ctx, interpretation).await.unwrap();
    let disc = facts.files[0].1.disc.as_ref().unwrap();
    assert_eq!(disc.kind, DiscKind::BluRay);
    assert_eq!(disc.root, image);
    assert_eq!(disc.file_count, 1);
    assert_eq!(disc.size_bytes, 50_008_686_592);
    assert_eq!(disc.duration.unwrap().as_millis(), 7_591_196);

    // In the tree, it's where it is, by its own name.
    let query = FileTreeQuery {
        work: None,
        folder: Some((source.uuid, "AnimeMovies".into())),
        identified: true,
        non_videos: true,
        after: None,
        limit: 50,
    };
    let entries = list_file_tree(&ctx, interpretation, &query)
        .await
        .unwrap()
        .entries;
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].name,
        "The.Wind.Rises.2013.Blu-ray.1080p.AVC.LPCM.2.0-Qux.iso"
    );
    assert_eq!(entries[0].path, image);
    assert!(!entries[0].is_folder && entries[0].disc.is_some());
}

mod common;

use common::wait_for_interpretations;
use tempfile::tempdir;
use wanshitong_app::commands::{create_data_source, delete_data_source, seed_data_sources};
use wanshitong_app::domain::{DataSourceConfig, FileSystemSource};
use wanshitong_app::queries::{
    list_data_sources, list_deleted_data_sources, list_ingests, wait_for_ingest,
};
use wanshitong_app::{AppContext, Config, CoreContext, DeclaredDataSource};

#[tokio::test]
async fn seeds_only_data_sources_no_source_is_named_for() {
    let tmp = tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("Movies")).unwrap();
    let declared = |name: &str| DeclaredDataSource {
        name: name.into(),
        root_path: tmp.path().join(name),
        follow_symlinks: false,
    };
    let config = Config {
        data_sources: vec![declared("Movies"), declared("TV"), declared("Old")],
        ..Default::default()
    };
    let core = CoreContext::new_in_memory().await.unwrap();
    let ctx = AppContext::new(core, config, "/fake/data");

    let elsewhere = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: "/elsewhere".into(),
        follow_symlinks: true,
    });
    create_data_source(&ctx, "TV", elsewhere.clone())
        .await
        .unwrap();
    let old = create_data_source(&ctx, "Old", elsewhere.clone())
        .await
        .unwrap();
    delete_data_source(&ctx, old.uuid).await.unwrap();
    wait_for_interpretations(&ctx).await;

    assert_eq!(seed_data_sources(&ctx).await.unwrap(), 1);

    let sources = list_data_sources(&ctx).await.unwrap();
    let movies = sources.iter().find(|s| s.name == "Movies").unwrap();
    assert_eq!(
        movies.config,
        DataSourceConfig::FileSystem(FileSystemSource {
            root_path: tmp.path().join("Movies"),
            follow_symlinks: false,
        })
    );
    // An existing source keeps its own config, and a deleted one stays deleted.
    let tv = sources.iter().find(|s| s.name == "TV").unwrap();
    assert_eq!(tv.config, elsewhere);
    assert!(sources.iter().all(|s| s.name != "Old"));
    assert_eq!(list_deleted_data_sources(&ctx).await.unwrap().len(), 1);

    // The new source is being ingested.
    let ingests = list_ingests(&ctx).await.unwrap();
    assert_eq!(ingests.len(), 1);
    assert_eq!(ingests[0].data_source.uuid, movies.uuid);
    wait_for_ingest(&ctx, ingests[0].ingest.uuid).await.unwrap();
    wait_for_interpretations(&ctx).await;

    // Seeding again changes nothing.
    assert_eq!(seed_data_sources(&ctx).await.unwrap(), 0);
    assert_eq!(list_data_sources(&ctx).await.unwrap().len(), 2);
}

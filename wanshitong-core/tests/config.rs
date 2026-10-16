use wanshitong_core::CoreContext;
use wanshitong_core::commands::{create_data_source, set_config, start_ingest};
use wanshitong_core::domain::{Config, DataSourceConfig, FileSystemSource};

async fn ingest_ffprobe_path(ctx: &CoreContext) -> String {
    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: "/media".into(),
        follow_symlinks: false,
    });
    let source = create_data_source(ctx, "Movies", config).await.unwrap();
    start_ingest(ctx, source.uuid).await.unwrap().ffprobe_path
}

#[tokio::test]
async fn a_new_database_ingests_with_the_default_config() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    assert_eq!(
        ingest_ffprobe_path(&ctx).await,
        Config::default().ffprobe_path
    );
}

#[tokio::test]
async fn setting_a_changed_config_takes_a_snapshot_that_new_ingests_run_with() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let config = Config {
        ffprobe_path: "/opt/ffmpeg/bin/ffprobe".into(),
        ..Config::default()
    };

    assert!(set_config(&ctx, &config).await.unwrap());
    assert_eq!(ingest_ffprobe_path(&ctx).await, config.ffprobe_path);
}

#[tokio::test]
async fn setting_the_config_already_in_effect_takes_no_snapshot() {
    let ctx = CoreContext::new_in_memory().await.unwrap();

    assert!(!set_config(&ctx, &Config::default()).await.unwrap());

    let config = Config {
        ffprobe_command: "-v error -print_format json -show_streams -i".into(),
        ..Config::default()
    };
    assert!(set_config(&ctx, &config).await.unwrap());
    assert!(!set_config(&ctx, &config).await.unwrap());
}

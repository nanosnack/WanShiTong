use std::time::Duration;
use tempfile::tempdir;
use wanshitong_app::commands::{
    IngestStart, assert_claim, create_data_source, create_view, set_view_auto_refresh, start_ingest,
};
use wanshitong_app::domain::PathId;
use wanshitong_app::domain::{
    ClaimPredicate, DataSourceConfig, ExternalId, FileRole, FileSystemSource, JellyfinView,
    PlexView, ViewConfig,
};
use wanshitong_app::interpretations::InterpretationState;
use wanshitong_app::queries::{get_interpretation, get_view, wait_for_ingest};
use wanshitong_app::views::ViewState;
use wanshitong_app::{AppContext, CoreContext};

fn jellyfin(root_path: &std::path::Path) -> ViewConfig {
    ViewConfig::Jellyfin(JellyfinView {
        root_path: root_path.into(),
    })
}

fn plex(root_path: &std::path::Path) -> ViewConfig {
    ViewConfig::Plex(PlexView {
        root_path: root_path.into(),
    })
}

#[tokio::test]
async fn views_are_refreshed_when_created_and_when_the_catalog_changes() {
    let media = tempdir().unwrap();
    let views = tempdir().unwrap();
    let core = CoreContext::new_in_memory().await.unwrap();
    let ctx = AppContext::new(core, Default::default(), "/fake/data");

    // Nothing to refresh from yet.
    let manual = create_view(
        &ctx,
        "manual",
        jellyfin(&views.path().join("manual")),
        false,
    )
    .await
    .unwrap();
    let before = create_view(&ctx, "before", jellyfin(&views.path().join("before")), true)
        .await
        .unwrap();
    assert!(before.latest_materialization.is_none());
    assert!(!views.path().join("before/Movies").exists());

    for file in ["heat.mkv", "wire.mkv"] {
        tokio::fs::write(media.path().join(file), "").await.unwrap();
    }
    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: media.path().into(),
        follow_symlinks: false,
    });
    let source = create_data_source(&ctx, "media", config).await.unwrap();
    let IngestStart::Started(ingest) = start_ingest(&ctx, source.uuid).await.unwrap() else {
        panic!("no ingest of it was running");
    };
    wait_for_ingest(&ctx, ingest).await.unwrap();
    // Not real videos, so make them a movie's main file and an episode by
    // assertion, identified so that views include them.
    for claim in [
        ClaimPredicate::FileRole(FileRole::Main),
        ClaimPredicate::MovieExternalId(ExternalId::Imdb("tt0113277".into())),
    ] {
        assert_claim(&ctx, &[PathId::new(source.uuid, "heat.mkv")], claim)
            .await
            .unwrap();
    }
    for claim in [
        ClaimPredicate::TvSeriesName("The Wire".into()),
        ClaimPredicate::Season(1),
        ClaimPredicate::EpisodeRange([1, 1]),
        ClaimPredicate::TvSeriesExternalId(ExternalId::Tvdb(79126)),
    ] {
        assert_claim(&ctx, &[PathId::new(source.uuid, "wire.mkv")], claim)
            .await
            .unwrap();
    }
    let interpretation = assert_claim(
        &ctx,
        &[PathId::new(source.uuid, "heat.mkv")],
        ClaimPredicate::MovieTitle("Heat".into()),
    )
    .await
    .unwrap();
    for _ in 0..200 {
        let state = get_interpretation(&ctx, interpretation)
            .await
            .unwrap()
            .unwrap()
            .state;
        if state != InterpretationState::Running {
            assert_eq!(state, InterpretationState::Succeeded);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Views that keep up with the catalog were refreshed as the
    // interpretation became canonical.
    let before = get_view(&ctx, before.uuid).await.unwrap().unwrap();
    assert_eq!(before.state, ViewState::UpToDate);
    assert!(views.path().join("before/Movies").exists());
    let manual_state = get_view(&ctx, manual.uuid).await.unwrap().unwrap().state;
    assert_eq!(manual_state, ViewState::NeverMaterialized);

    // Keeping up with the catalog brings a view up to date right away.
    set_view_auto_refresh(&ctx, manual.uuid, true)
        .await
        .unwrap();
    let manual = get_view(&ctx, manual.uuid).await.unwrap().unwrap();
    assert!(manual.view.auto_refresh);
    assert_eq!(manual.state, ViewState::UpToDate);

    let after = create_view(&ctx, "after", jellyfin(&views.path().join("after")), true)
        .await
        .unwrap();
    let stats = after.latest_materialization.unwrap().result.unwrap();
    assert_eq!(stats.links_created, 2);
    let link = views
        .path()
        .join("after/Movies/Heat [imdbid-tt0113277]/Heat [imdbid-tt0113277].mkv");
    assert_eq!(
        std::fs::read_link(link).unwrap(),
        media.path().join("heat.mkv")
    );
    let link = views
        .path()
        .join("after/Shows/The Wire [tvdbid-79126]/Season 01/The Wire S01E01.mkv");
    assert_eq!(
        std::fs::read_link(link).unwrap(),
        media.path().join("wire.mkv")
    );

    // A Plex view names the same files by Plex's rules.
    create_view(&ctx, "plex", plex(&views.path().join("plex")), true)
        .await
        .unwrap();
    let link = views
        .path()
        .join("plex/Movies/Heat {imdb-tt0113277}/Heat {imdb-tt0113277}.mkv");
    assert_eq!(
        std::fs::read_link(link).unwrap(),
        media.path().join("heat.mkv")
    );
    let link = views
        .path()
        .join("plex/TV Shows/The Wire {tvdb-79126}/Season 01/The Wire S01E01.mkv");
    assert_eq!(
        std::fs::read_link(link).unwrap(),
        media.path().join("wire.mkv")
    );
}

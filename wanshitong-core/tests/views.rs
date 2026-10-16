mod common;

use common::{file_id, ingest_files};
use jiff::Timestamp;
use wanshitong_core::commands::{
    NewCatalog, NewEpisode, NewMovie, NewMovieExpression, NewMovieFile, NewTvSeries,
    NewViewMaterialization, create_data_source, create_interpretation, create_view, delete_view,
    record_catalog, record_view_materialization,
};
use wanshitong_core::domain::{
    DataSourceConfig, FileRole, FileSystemSource, JellyfinView, PlexView, ViewConfig,
    ViewEpisodeFile, ViewMaterialization, ViewMovie, ViewMovieFile, ViewSyncStats, ViewTvSeries,
};
use wanshitong_core::queries::{
    get_view, list_movies_for_view, list_tv_series_for_view, list_views,
};
use wanshitong_core::{CoreContext, CoreErrorContext, NameError};

fn jellyfin(root_path: &str) -> ViewConfig {
    ViewConfig::Jellyfin(JellyfinView {
        root_path: root_path.into(),
    })
}

fn plex(root_path: &str) -> ViewConfig {
    ViewConfig::Plex(PlexView {
        root_path: root_path.into(),
    })
}

#[tokio::test]
async fn views_may_not_overlap_data_sources_or_each_other() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    // `ingest_files` roots its data source at /media.
    ingest_files(&ctx, "media", &[]).await;
    create_view(&ctx, "jellyfin", jellyfin("/srv/view"), true)
        .await
        .unwrap();

    for root in ["/media", "/media/view", "/"] {
        let err = create_view(&ctx, "other", jellyfin(root), true)
            .await
            .unwrap_err();
        assert!(
            matches!(err.current_context(), CoreErrorContext::ViewOverlapsDataSource { data_source, .. } if data_source == "media"),
            "{root}: {err}"
        );
    }
    for root in ["/srv/view", "/srv/view/Movies", "/srv"] {
        let err = create_view(&ctx, "other", jellyfin(root), true)
            .await
            .unwrap_err();
        assert!(
            matches!(err.current_context(), CoreErrorContext::ViewOverlapsView { view, .. } if view == "jellyfin"),
            "{root}: {err}"
        );
    }
    // Only a component-wise prefix counts as overlapping.
    create_view(&ctx, "sibling", jellyfin("/srv/view2"), true)
        .await
        .unwrap();

    let data_source = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: "/srv/view/Movies".into(),
        follow_symlinks: true,
    });
    let err = create_data_source(&ctx, "inside view", data_source)
        .await
        .unwrap_err();
    assert!(matches!(
        err.current_context(),
        CoreErrorContext::DataSourceOverlapsView { .. }
    ));
}

#[tokio::test]
async fn plex_views_are_kept_and_kept_apart_like_jellyfin_views() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let jellyfin_view = create_view(&ctx, "jellyfin", jellyfin("/srv/jellyfin"), true)
        .await
        .unwrap();
    let plex_view = create_view(&ctx, "plex", plex("/srv/plex"), false)
        .await
        .unwrap();
    assert_eq!(
        list_views(&ctx).await.unwrap(),
        vec![jellyfin_view, plex_view],
    );

    let err = create_view(&ctx, "other", jellyfin("/srv/plex/Movies"), true)
        .await
        .unwrap_err();
    assert!(
        matches!(err.current_context(), CoreErrorContext::ViewOverlapsView { view, .. } if view == "plex")
    );
    let err = create_view(&ctx, "other", plex("/srv/jellyfin/Shows"), true)
        .await
        .unwrap_err();
    assert!(
        matches!(err.current_context(), CoreErrorContext::ViewOverlapsView { view, .. } if view == "jellyfin")
    );

    let data_source = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: "/srv/plex/TV Shows".into(),
        follow_symlinks: true,
    });
    let err = create_data_source(&ctx, "inside view", data_source)
        .await
        .unwrap_err();
    assert!(
        matches!(err.current_context(), CoreErrorContext::DataSourceOverlapsView { view, .. } if view == "plex")
    );
}

#[tokio::test]
async fn rejects_bad_view_names_and_roots() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    create_view(&ctx, "jellyfin", jellyfin("/srv/a"), true)
        .await
        .unwrap();

    let err = create_view(&ctx, " jellyfin ", jellyfin("/srv/b"), true)
        .await
        .unwrap_err();
    assert!(matches!(
        err.current_context(),
        CoreErrorContext::ViewNameExists(_)
    ));

    let err = create_view(&ctx, "  ", jellyfin("/srv/b"), true)
        .await
        .unwrap_err();
    assert!(matches!(
        err.current_context(),
        CoreErrorContext::InvalidViewName(NameError::Empty)
    ));

    for root in ["srv/b", "/srv/a/../b"] {
        let err = create_view(&ctx, "other", jellyfin(root), true)
            .await
            .unwrap_err();
        assert!(
            matches!(
                err.current_context(),
                CoreErrorContext::InvalidViewRootPath(_)
            ),
            "{root}"
        );
    }
}

#[tokio::test]
async fn lists_movies_with_absolute_file_paths_and_their_copies() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let paths = [
        "Heat/heat.mkv",
        "Heat/trailer.mkv",
        "Heat Extended/heat.cd1.mkv",
        "Heat Extended/heat.cd2.mkv",
        "Heat BD/BDMV/index.bdmv",
        "Heat BD/BDMV/STREAM/00000.m2ts",
    ];
    let sources = [ingest_files(&ctx, "media", &paths).await];
    let interpretation = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let file = |rel_path| file_id(&ctx, interpretation, rel_path);
    let file = async |rel_path, role, expression, manifestation, part| NewMovieFile {
        file_id: file(rel_path).await,
        role: Some(role),
        expression,
        manifestation,
        part,
    };
    record_catalog(
        &ctx,
        interpretation,
        NewCatalog {
            movies: vec![NewMovie {
                title: "Heat".into(),
                expressions: vec![
                    NewMovieExpression { edition: None },
                    NewMovieExpression {
                        edition: Some("Extended".into()),
                    },
                ],
                tmdb_id: Some(949),
                release_year: Some(1995),
                files: vec![
                    file("Heat/heat.mkv", FileRole::Main, Some(0), Some(0), None).await,
                    file("Heat/trailer.mkv", FileRole::Extra, None, None, None).await,
                    file(
                        "Heat Extended/heat.cd1.mkv",
                        FileRole::Main,
                        Some(1),
                        Some(1),
                        Some(1),
                    )
                    .await,
                    file(
                        "Heat Extended/heat.cd2.mkv",
                        FileRole::Main,
                        Some(1),
                        Some(1),
                        Some(2),
                    )
                    .await,
                    file(
                        "Heat BD/BDMV/index.bdmv",
                        FileRole::Main,
                        Some(0),
                        Some(2),
                        None,
                    )
                    .await,
                ],
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let file = |role, manifestation, part, edition: Option<&str>, path: &str| ViewMovieFile {
        role: Some(role),
        manifestation,
        part,
        edition: edition.map(String::from),
        disc: false,
        path: path.into(),
    };
    assert_eq!(
        list_movies_for_view(&ctx, interpretation).await.unwrap(),
        vec![ViewMovie {
            title: "Heat".into(),
            release_year: Some(1995),
            tmdb_id: Some(949),
            imdb_id: None,
            files: vec![
                file(FileRole::Main, Some(0), None, None, "/media/Heat/heat.mkv"),
                file(FileRole::Extra, None, None, None, "/media/Heat/trailer.mkv"),
                // Both parts of one manifestation.
                file(
                    FileRole::Main,
                    Some(1),
                    Some(1),
                    Some("Extended"),
                    "/media/Heat Extended/heat.cd1.mkv",
                ),
                file(
                    FileRole::Main,
                    Some(1),
                    Some(2),
                    Some("Extended"),
                    "/media/Heat Extended/heat.cd2.mkv",
                ),
                // A disc, by its index file.
                ViewMovieFile {
                    disc: true,
                    ..file(
                        FileRole::Main,
                        Some(2),
                        None,
                        None,
                        "/media/Heat BD/BDMV/index.bdmv",
                    )
                },
            ],
        }],
    );
}

#[tokio::test]
async fn lists_tv_series_with_absolute_episode_and_extra_paths() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let paths = [
        "op/092.mkv",
        "op/NCOP.mkv",
        "op/S01E01-02.mkv",
        "op/manifestation/S01E01.mkv",
    ];
    let sources = [ingest_files(&ctx, "media", &paths).await];
    let interpretation = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let id = |path| file_id(&ctx, interpretation, path);
    let episode = |file_id, season_number, episode_range| NewEpisode {
        file_id,
        season_number,
        episode_range,
        episode_title: None,
    };
    record_catalog(
        &ctx,
        interpretation,
        NewCatalog {
            tv_series: vec![NewTvSeries {
                name: "One Piece".into(),
                tmdb_id: Some(37854),
                episodes: vec![
                    episode(id("op/092.mkv").await, None, [92, 92]),
                    episode(id("op/S01E01-02.mkv").await, Some(1), [1, 2]),
                    episode(id("op/manifestation/S01E01.mkv").await, Some(1), [1, 1]),
                ],
                extras: vec![id("op/NCOP.mkv").await],
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let file = |season, episode, path: &str| ViewEpisodeFile {
        season,
        episode,
        path: path.into(),
    };
    assert_eq!(
        list_tv_series_for_view(&ctx, interpretation).await.unwrap(),
        vec![ViewTvSeries {
            name: "One Piece".into(),
            tmdb_id: Some(37854),
            tvdb_id: None,
            imdb_id: None,
            // A double episode's file is listed for each of its episodes.
            episode_files: vec![
                file(Some(1), 1, "/media/op/S01E01-02.mkv"),
                file(Some(1), 1, "/media/op/manifestation/S01E01.mkv"),
                file(Some(1), 2, "/media/op/S01E01-02.mkv"),
                file(None, 92, "/media/op/092.mkv"),
            ],
            extras: vec!["/media/op/NCOP.mkv".into()],
        }],
    );
}

#[tokio::test]
async fn views_show_their_latest_materialization() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &[]).await];
    let interpretation = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let view = create_view(&ctx, "jellyfin", jellyfin("/srv/view"), true)
        .await
        .unwrap();
    assert_eq!(list_views(&ctx).await.unwrap(), vec![view.clone()]);

    let at = Timestamp::from_second(1_700_000_000).unwrap();
    let stats = ViewSyncStats {
        links_created: 3,
        links_updated: 2,
        links_removed: 1,
    };
    for result in [Err("disk full".to_string()), Ok(stats)] {
        record_view_materialization(
            &ctx,
            NewViewMaterialization {
                view: view.uuid,
                interpretation_id: Some(interpretation),
                started_at: at,
                stopped_at: at,
                result,
            },
        )
        .await
        .unwrap();
    }

    let latest = get_view(&ctx, view.uuid)
        .await
        .unwrap()
        .unwrap()
        .latest_materialization;
    assert_eq!(
        latest,
        Some(ViewMaterialization {
            interpretation_id: Some(interpretation),
            started_at: at,
            stopped_at: at,
            result: Ok(stats),
        }),
    );
}

#[tokio::test]
async fn deleting_a_view_frees_its_name_and_root() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &[]).await];
    let interpretation = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let view = create_view(&ctx, "jellyfin", jellyfin("/srv/view"), true)
        .await
        .unwrap();
    let at = Timestamp::from_second(1_700_000_000).unwrap();
    record_view_materialization(
        &ctx,
        NewViewMaterialization {
            view: view.uuid,
            interpretation_id: Some(interpretation),
            started_at: at,
            stopped_at: at,
            result: Ok(ViewSyncStats::default()),
        },
    )
    .await
    .unwrap();

    delete_view(&ctx, view.uuid).await.unwrap();

    assert_eq!(list_views(&ctx).await.unwrap(), vec![]);
    assert!(matches!(
        delete_view(&ctx, view.uuid).await.unwrap_err().current_context(),
        CoreErrorContext::ViewNotFound(uuid) if *uuid == view.uuid
    ));
    create_view(&ctx, "jellyfin", jellyfin("/srv/view"), true)
        .await
        .unwrap();
}

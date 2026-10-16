mod common;

use common::{file_id, ingest_files};
use std::path::Path;
use wanshitong_core::CoreContext;
use wanshitong_core::commands::{
    NewCatalog, NewEpisode, NewMovie, NewMovieExpression, NewMovieFile, NewTvSeries,
    create_interpretation, record_catalog,
};
use wanshitong_core::domain::{
    ClaimPredicate, ClaimSource, ExternalId, FileRole, Heuristic, KbQueryEvent, KbSearchResult,
    PathId, RankedClaimPredicate, SubjectAuditLog,
};
use wanshitong_core::queries::{FileSearchQuery, get_file, list_file_details, list_files};

#[tokio::test]
async fn a_file_says_what_it_is_in_the_catalog_and_why() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let paths = ["heat.mkv", "S01E01-02.mkv", "NCOP 01.mkv"];
    let ingested = ingest_files(&ctx, "media", &paths).await;
    let source = ingested.0;
    let interpretation = create_interpretation(&ctx, "test", &[ingested])
        .await
        .unwrap()
        .row_id;
    let id = async |path| file_id(&ctx, interpretation, path).await;
    let geass = KbSearchResult {
        id: ExternalId::Tmdb(31724),
        title: "Code Geass: Lelouch of the Rebellion".into(),
        year: Some(2006),
    };
    let season = RankedClaimPredicate::new(
        ClaimSource::Heuristic(Heuristic::FileNameSeasonEpisode),
        3.0,
        ClaimPredicate::Season(1),
    );

    record_catalog(
        &ctx,
        interpretation,
        NewCatalog {
            movies: vec![NewMovie {
                title: "Heat".into(),
                expressions: vec![NewMovieExpression { edition: None }],
                release_year: Some(1995),
                files: vec![NewMovieFile {
                    file_id: id("heat.mkv").await,
                    role: Some(FileRole::Main),
                    expression: Some(0),
                    manifestation: Some(0),
                    part: None,
                }],
                ..Default::default()
            }],
            tv_series: vec![NewTvSeries {
                name: "Code Geass".into(),
                episodes: vec![NewEpisode {
                    file_id: id("S01E01-02.mkv").await,
                    season_number: Some(1),
                    episode_range: [1, 2],
                    episode_title: None,
                }],
                ..Default::default()
            }],
            // Claimed about, but in neither.
            audit_logs: vec![(
                id("NCOP 01.mkv").await,
                SubjectAuditLog {
                    claims: vec![season.clone()],
                    kb_queries: vec![KbQueryEvent::TvSeriesSearch {
                        query: "ncop 01".into(),
                        results: vec![geass.clone()],
                    }],
                },
            )],
            unidentified_videos: vec![],
            videos: vec![],
        },
    )
    .await
    .unwrap();
    let file = async |path: &str| {
        get_file(&ctx, interpretation, source, Path::new(path))
            .await
            .unwrap()
    };

    let heat = file("heat.mkv").await.unwrap();
    assert_eq!(heat.data_source_name, "media");
    let movies: Vec<_> = heat
        .movies
        .iter()
        .map(|m| (m.title.as_str(), m.role))
        .collect();
    assert_eq!(movies, vec![("Heat", Some(FileRole::Main))]);
    assert!(heat.episodes.is_empty());
    assert!(heat.audit_log.is_none());

    let double = file("S01E01-02.mkv").await.unwrap();
    let episodes: Vec<_> = double
        .episodes
        .iter()
        .map(|e| (e.series_name.as_str(), e.season_number, e.episode_number))
        .collect();
    assert_eq!(
        episodes,
        vec![("Code Geass", Some(1), 1), ("Code Geass", Some(1), 2)]
    );

    let ncop = file("NCOP 01.mkv").await.unwrap();
    assert!(ncop.movies.is_empty() && ncop.episodes.is_empty());
    let log = ncop.audit_log.unwrap();
    let claims: Vec<_> = log.claims.into_iter().map(|c| c.pred).collect();
    assert_eq!(claims, vec![season.pred]);
    // With what the search found.
    assert!(matches!(
        &log.kb_queries[..],
        [KbQueryEvent::TvSeriesSearch { results, .. }] if *results == vec![geass]
    ));

    assert!(file("elsewhere.mkv").await.is_none());
}

#[tokio::test]
async fn lists_an_interpretations_unidentified_videos() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let paths = ["heat.mkv", "S01E01.mkv", "NCOP 01.mkv", "notes.txt"];
    let ingested = ingest_files(&ctx, "media", &paths).await;
    let ours = create_interpretation(&ctx, "ours", &[ingested])
        .await
        .unwrap()
        .row_id;
    // Another interpretation of the same files, which identified more of them.
    let theirs = create_interpretation(&ctx, "theirs", &[ingested])
        .await
        .unwrap()
        .row_id;
    let catalog = async |interpretation, unidentified: &[&str]| {
        let mut unidentified_videos = Vec::new();
        for path in unidentified {
            unidentified_videos.push(file_id(&ctx, interpretation, path).await);
        }
        record_catalog(
            &ctx,
            interpretation,
            NewCatalog {
                unidentified_videos,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    };
    catalog(ours, &["S01E01.mkv", "NCOP 01.mkv"]).await;
    catalog(theirs, &["NCOP 01.mkv"]).await;

    let unidentified = list_files(&ctx, ours, &FileSearchQuery::default())
        .await
        .unwrap()
        .files;
    let paths: Vec<_> = unidentified.iter().map(|f| f.rel_path.as_str()).collect();
    assert_eq!(paths, vec!["NCOP 01.mkv", "S01E01.mkv"]);
}

#[tokio::test]
async fn lists_files_in_the_order_asked_once_each() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let media = ingest_files(&ctx, "media", &["heat.mkv", "S01E01.mkv"]).await;
    let other = ingest_files(&ctx, "other", &["heat.mkv"]).await;
    let interpretation = create_interpretation(&ctx, "test", &[media, other])
        .await
        .unwrap()
        .row_id;
    record_catalog(
        &ctx,
        interpretation,
        NewCatalog {
            tv_series: vec![NewTvSeries {
                name: "Code Geass".into(),
                episodes: vec![NewEpisode {
                    file_id: file_id(&ctx, interpretation, "S01E01.mkv").await,
                    season_number: Some(1),
                    episode_range: [1, 1],
                    episode_title: None,
                }],
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Out of path order, with a repeat, a file nobody has and the same path
    // in two data sources.
    let paths = [
        PathId::new(media.0, "S01E01.mkv"),
        PathId::new(other.0, "heat.mkv"),
        PathId::new(media.0, "S01E01.mkv"),
        PathId::new(media.0, "elsewhere.mkv"),
        PathId::new(media.0, "heat.mkv"),
    ];
    let files = list_file_details(&ctx, interpretation, &paths)
        .await
        .unwrap();
    let listed: Vec<_> = files
        .iter()
        .map(|f| {
            (
                f.data_source_name.as_str(),
                f.rel_path.to_str().unwrap(),
                f.episodes.len(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        vec![
            ("media", "S01E01.mkv", 1),
            ("other", "heat.mkv", 0),
            ("media", "heat.mkv", 0),
        ]
    );
}

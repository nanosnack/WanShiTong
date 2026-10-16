mod common;

use common::{file_id, ingest_files};
use std::path::PathBuf;
use wanshitong_core::commands::{
    CatalogError, NewCatalog, NewEpisode, NewTvSeries, create_interpretation, record_catalog,
};
use wanshitong_core::domain::{
    Date, FileMeasurementRowId, InterpretationRowId, TvSeriesDetails, TvSeriesKbData,
};
use wanshitong_core::queries::{TvSeriesSearchQuery, get_file, get_tv_series, list_tv_series};
use wanshitong_core::{CoreContext, CoreErrorContext};

fn episode(file_id: FileMeasurementRowId, season: u32, episodes: [u32; 2]) -> NewEpisode {
    NewEpisode {
        file_id,
        season_number: Some(season),
        episode_range: episodes,
        episode_title: None,
    }
}

async fn interpretation_of(ctx: &CoreContext, rel_paths: &[&str]) -> InterpretationRowId {
    let sources = [ingest_files(ctx, "media", rel_paths).await];
    create_interpretation(ctx, "test", &sources)
        .await
        .unwrap()
        .row_id
}

/// Each season's number, with each of its episodes' numbers and file paths.
type Seasons = Vec<(Option<u32>, Vec<(u32, Vec<PathBuf>)>)>;

fn seasons(series: &TvSeriesDetails) -> Seasons {
    series
        .seasons
        .iter()
        .map(|season| {
            let episodes = season
                .episodes
                .iter()
                .map(|e| {
                    (
                        e.number,
                        e.files.iter().map(|f| f.rel_path.clone()).collect(),
                    )
                })
                .collect();
            (season.number, episodes)
        })
        .collect()
}

#[tokio::test]
async fn series_are_listed_by_name_with_their_seasons_and_episodes() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let paths = [
        "cg/S01E01.mkv",
        "cg/S01E02.mkv",
        "cg/S02E01-02.mkv",
        "copy/S01E01.mkv",
        "wire/S01E01.mkv",
    ];
    let interpretation = interpretation_of(&ctx, &paths).await;
    let id = |path| file_id(&ctx, interpretation, path);

    record_catalog(
        &ctx,
        interpretation,
        NewCatalog {
            tv_series: vec![
                NewTvSeries {
                    name: "The Wire".into(),
                    tmdb_id: Some(1438),
                    imdb_id: Some("tt0306414".into()),
                    kb_data: TvSeriesKbData {
                        original_name: Some("The Wire".into()),
                        original_language: Some("en".into()),
                        tagline: Some("Listen carefully.".into()),
                        overview: Some("Told from the points of view of both the Baltimore homicide and narcotics detectives and their targets.".into()),
                        first_air_date: Some(Date {
                            year: 2002,
                            month: 6,
                            day: 2,
                        }),
                        poster_path: Some("/4lbclFySvugI51fwsyxBTOm4DqK.jpg".into()),
                    },
                    episodes: vec![episode(id("wire/S01E01.mkv").await, 1, [1, 1])],
                    ..Default::default()
                },
                NewTvSeries {
                    name: "Code Geass".into(),
                    tmdb_id: Some(31724),
                    episodes: vec![
                        episode(id("cg/S02E01-02.mkv").await, 2, [1, 2]),
                        episode(id("cg/S01E02.mkv").await, 1, [2, 2]),
                        episode(id("cg/S01E01.mkv").await, 1, [1, 1]),
                        episode(id("copy/S01E01.mkv").await, 1, [1, 1]),
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let list = list_tv_series(&ctx, interpretation, &TvSeriesSearchQuery::default())
        .await
        .unwrap();
    // "The Wire" sorts among the Ws.
    let summaries: Vec<_> = list
        .series
        .iter()
        .map(|s| (s.name.as_str(), s.season_count, s.episode_count))
        .collect();
    assert_eq!(summaries, vec![("Code Geass", 2, 4), ("The Wire", 1, 1)]);

    let code_geass = get_tv_series(&ctx, list.series[0].uuid)
        .await
        .unwrap()
        .unwrap();
    // A double episode's file is a file of both episodes.
    assert_eq!(
        seasons(&code_geass),
        vec![
            (
                Some(1),
                vec![
                    (1, vec!["cg/S01E01.mkv".into(), "copy/S01E01.mkv".into()]),
                    (2, vec!["cg/S01E02.mkv".into()]),
                ]
            ),
            (
                Some(2),
                vec![
                    (1, vec!["cg/S02E01-02.mkv".into()]),
                    (2, vec!["cg/S02E01-02.mkv".into()]),
                ]
            ),
        ]
    );

    let the_wire = get_tv_series(&ctx, list.series[1].uuid)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(the_wire.imdb_id.as_deref(), Some("tt0306414"));
    assert_eq!(the_wire.original_name.as_deref(), Some("The Wire"));
    assert_eq!(the_wire.original_language.as_deref(), Some("en"));
    assert_eq!(the_wire.first_air_year, Some(2002));
    assert_eq!(the_wire.tagline.as_deref(), Some("Listen carefully."));
    assert!(the_wire.overview.is_some());
    assert_eq!(code_geass.first_air_year, None);
    let poster = Some("/4lbclFySvugI51fwsyxBTOm4DqK.jpg");
    assert_eq!(the_wire.tmdb_poster_path.as_deref(), poster);
    assert_eq!(list.series[1].tmdb_poster_path.as_deref(), poster);
    assert_eq!(list.series[0].tmdb_poster_path, None);

    let search = TvSeriesSearchQuery {
        name: Some("wire".into()),
        ..Default::default()
    };
    let names: Vec<_> = list_tv_series(&ctx, interpretation, &search)
        .await
        .unwrap()
        .series
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(names, vec!["The Wire"]);
}

#[tokio::test]
async fn pages_through_every_series_once() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let paths: Vec<String> = (0..5).map(|i| format!("{i}.mkv")).collect();
    let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    let interpretation = interpretation_of(&ctx, &paths).await;
    let mut tv_series = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        tv_series.push(NewTvSeries {
            // Some share a name, so the cursor has to tell them apart.
            name: format!("Series {}", i / 2),
            tmdb_id: Some(i as u32 + 1),
            episodes: vec![episode(
                file_id(&ctx, interpretation, path).await,
                1,
                [1, 1],
            )],
            ..Default::default()
        });
    }
    record_catalog(
        &ctx,
        interpretation,
        NewCatalog {
            tv_series,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = list_tv_series(
            &ctx,
            interpretation,
            &TvSeriesSearchQuery {
                cursor,
                limit: 2,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        seen.extend(page.series.into_iter().map(|s| s.uuid));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 5);
}

#[tokio::test]
async fn rejects_series_that_share_an_id() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let interpretation = interpretation_of(&ctx, &["a.mkv", "b.mkv"]).await;
    let series = |name: &str, file_id| NewTvSeries {
        name: name.into(),
        tmdb_id: Some(31724),
        episodes: vec![episode(file_id, 1, [1, 1])],
        ..Default::default()
    };

    let err = record_catalog(
        &ctx,
        interpretation,
        NewCatalog {
            tv_series: vec![
                series("Code Geass", file_id(&ctx, interpretation, "a.mkv").await),
                series(
                    "Code Geass R2",
                    file_id(&ctx, interpretation, "b.mkv").await,
                ),
            ],
            ..Default::default()
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(
        err.current_context(),
        CoreErrorContext::InvalidCatalog(CatalogError::SharedTvSeriesId { id, .. }) if id == "TMDB ID 31724"
    ));
}

#[tokio::test]
async fn a_series_is_in_the_data_sources_of_its_episodes_and_extras() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [
        ingest_files(&ctx, "shows", &["cg/S01E01.mkv"]).await,
        ingest_files(&ctx, "backup", &["cg-backup/S01E01.mkv"]).await,
        ingest_files(&ctx, "extras", &["cg/NCOP.mkv"]).await,
        ingest_files(&ctx, "other", &["other.mkv"]).await,
    ];
    let interpretation = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let id = |path| file_id(&ctx, interpretation, path);

    record_catalog(
        &ctx,
        interpretation,
        NewCatalog {
            tv_series: vec![NewTvSeries {
                name: "Code Geass".into(),
                tmdb_id: Some(31724),
                episodes: vec![
                    episode(id("cg/S01E01.mkv").await, 1, [1, 1]),
                    episode(id("cg-backup/S01E01.mkv").await, 1, [1, 1]),
                ],
                extras: vec![id("cg/NCOP.mkv").await],
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let list = list_tv_series(&ctx, interpretation, &TvSeriesSearchQuery::default())
        .await
        .unwrap();
    let code_geass = get_tv_series(&ctx, list.series[0].uuid)
        .await
        .unwrap()
        .unwrap();
    let names: Vec<_> = code_geass
        .data_sources
        .iter()
        .map(|source| source.name.as_str())
        .collect();
    assert_eq!(names, vec!["backup", "extras", "shows"]);
}

#[tokio::test]
async fn extras_belong_to_their_series() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let paths = [
        "cg/S01E01.mkv",
        "cg/NCOP 01.mkv",
        "cg/NCED 01.mkv",
        "akito/NCOP.mkv",
        "notes.txt",
    ];
    let ingested = ingest_files(&ctx, "media", &paths).await;
    let interpretation = create_interpretation(&ctx, "test", &[ingested])
        .await
        .unwrap()
        .row_id;
    let id = |path| file_id(&ctx, interpretation, path);

    record_catalog(
        &ctx,
        interpretation,
        NewCatalog {
            tv_series: vec![
                NewTvSeries {
                    name: "Code Geass".into(),
                    tmdb_id: Some(31724),
                    episodes: vec![episode(id("cg/S01E01.mkv").await, 1, [1, 1])],
                    extras: vec![id("cg/NCED 01.mkv").await, id("cg/NCOP 01.mkv").await],
                    ..Default::default()
                },
                // Only its extras are here.
                NewTvSeries {
                    name: "Akito the Exiled".into(),
                    tmdb_id: Some(62555),
                    extras: vec![id("akito/NCOP.mkv").await],
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let list = list_tv_series(&ctx, interpretation, &TvSeriesSearchQuery::default())
        .await
        .unwrap();
    let summaries: Vec<_> = list
        .series
        .iter()
        .map(|s| (s.name.as_str(), s.season_count, s.episode_count))
        .collect();
    assert_eq!(
        summaries,
        vec![("Akito the Exiled", 0, 0), ("Code Geass", 1, 1)]
    );

    let code_geass = get_tv_series(&ctx, list.series[1].uuid)
        .await
        .unwrap()
        .unwrap();
    let extras: Vec<_> = code_geass
        .extras
        .iter()
        .map(|f| f.rel_path.clone())
        .collect();
    assert_eq!(
        extras,
        vec![PathBuf::from("cg/NCED 01.mkv"), "cg/NCOP 01.mkv".into()]
    );

    let ncop = get_file(&ctx, interpretation, ingested.0, "cg/NCOP 01.mkv".as_ref())
        .await
        .unwrap()
        .unwrap();
    let series: Vec<_> = ncop
        .series_extras
        .iter()
        .map(|s| s.series_name.as_str())
        .collect();
    assert_eq!(series, vec!["Code Geass"]);
    assert!(ncop.episodes.is_empty());
}

#[tokio::test]
async fn episodes_of_an_unknown_season_are_kept_together_after_the_seasons() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let paths = [
        "op/One.Piece.S00E36.mkv",
        "op/One.Piece.E92.mkv",
        "op/One.Piece.E93-E95.mkv",
    ];
    let ingested = ingest_files(&ctx, "media", &paths).await;
    let interpretation = create_interpretation(&ctx, "test", &[ingested])
        .await
        .unwrap()
        .row_id;
    let id = |path| file_id(&ctx, interpretation, path);
    let absolute = |file_id, episodes| NewEpisode {
        file_id,
        season_number: None,
        episode_range: episodes,
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
                    absolute(id("op/One.Piece.E93-E95.mkv").await, [93, 95]),
                    episode(id("op/One.Piece.S00E36.mkv").await, 0, [36, 36]),
                    absolute(id("op/One.Piece.E92.mkv").await, [92, 92]),
                ],
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let list = list_tv_series(&ctx, interpretation, &TvSeriesSearchQuery::default())
        .await
        .unwrap();
    // Episodes of an unknown season aren't a season of their own.
    assert_eq!(
        (list.series[0].season_count, list.series[0].episode_count),
        (1, 5)
    );

    let one_piece = get_tv_series(&ctx, list.series[0].uuid)
        .await
        .unwrap()
        .unwrap();
    let episodes = |season: &[(u32, Vec<PathBuf>)]| -> Vec<u32> {
        season.iter().map(|(number, _)| *number).collect()
    };
    let seasons: Vec<_> = seasons(&one_piece)
        .into_iter()
        .map(|(number, season)| (number, episodes(&season)))
        .collect();
    assert_eq!(
        seasons,
        vec![(Some(0), vec![36]), (None, vec![92, 93, 94, 95])]
    );

    let file = get_file(
        &ctx,
        interpretation,
        ingested.0,
        "op/One.Piece.E93-E95.mkv".as_ref(),
    )
    .await
    .unwrap()
    .unwrap();
    let numbers: Vec<_> = file
        .episodes
        .iter()
        .map(|e| (e.season_number, e.episode_number))
        .collect();
    assert_eq!(numbers, vec![(None, 93), (None, 94), (None, 95)]);
}

#[tokio::test]
async fn a_series_disc_is_kept_apart_from_its_extras() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    // A disc's index file: ingest_files gives it no other files, so it's no
    // disc of its own here, which a series' disc needn't be to be recorded.
    let sources =
        [ingest_files(&ctx, "media", &["show/NCOP.mkv", "show/D1/BDMV/index.bdmv"]).await];
    let interpretation = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let id = |path| file_id(&ctx, interpretation, path);
    record_catalog(
        &ctx,
        interpretation,
        NewCatalog {
            tv_series: vec![NewTvSeries {
                name: "Show".into(),
                tmdb_id: Some(1),
                extras: vec![id("show/NCOP.mkv").await],
                discs: vec![id("show/D1/BDMV/index.bdmv").await],
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let list = list_tv_series(&ctx, interpretation, &TvSeriesSearchQuery::default())
        .await
        .unwrap();
    let series = get_tv_series(&ctx, list.series[0].uuid)
        .await
        .unwrap()
        .unwrap();
    let paths = |files: &[wanshitong_core::domain::TvSeriesFile]| -> Vec<PathBuf> {
        files.iter().map(|file| file.rel_path.clone()).collect()
    };
    assert_eq!(paths(&series.extras), [PathBuf::from("show/NCOP.mkv")]);
    assert_eq!(
        paths(&series.discs),
        [PathBuf::from("show/D1/BDMV/index.bdmv")]
    );

    let disc = get_file(
        &ctx,
        interpretation,
        sources[0].0,
        "show/D1/BDMV/index.bdmv".as_ref(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(disc.series_extras[0].disc);
}

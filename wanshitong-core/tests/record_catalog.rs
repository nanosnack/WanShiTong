mod common;

use common::{file_id, ingest_files};
use std::path::PathBuf;
use wanshitong_core::commands::{
    CatalogError, NewCatalog, NewMovie, NewMovieExpression, NewMovieFile, create_interpretation,
    record_catalog,
};
use wanshitong_core::domain::{
    Date, FileMeasurementRowId, FileRole, InterpretationRowId, MovieKbData,
};
use wanshitong_core::queries::{MovieSearchQuery, get_movie, list_movies};
use wanshitong_core::{CoreContext, CoreErrorContext};

fn heat(files: Vec<NewMovieFile>) -> NewMovie {
    NewMovie {
        title: "Heat".into(),
        expressions: vec![NewMovieExpression { edition: None }],
        tmdb_id: Some(949),
        files,
        ..Default::default()
    }
}

async fn movie_files(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
) -> Vec<(PathBuf, Option<FileRole>)> {
    let movies = list_movies(ctx, interpretation_id, &MovieSearchQuery::default())
        .await
        .unwrap()
        .movies;
    assert_eq!(movies.len(), 1);
    let details = get_movie(ctx, movies[0].uuid).await.unwrap().unwrap();
    let mut files: Vec<_> = details
        .files
        .into_iter()
        .map(|f| (f.rel_path, f.role))
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

#[tokio::test]
async fn each_interpretation_has_its_own_movies() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &["heat.mkv", "heat-trailer.mkv"]).await];
    let first = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let second = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let main = file_id(&ctx, first, "heat.mkv").await;
    let trailer = file_id(&ctx, first, "heat-trailer.mkv").await;

    record_catalog(
        &ctx,
        first,
        NewCatalog {
            movies: vec![heat(vec![NewMovieFile {
                file_id: main,
                role: Some(FileRole::Main),
                expression: Some(0),
                manifestation: Some(0),
                part: None,
            }])],
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // The same TMDB ID as the first interpretation's movie.
    record_catalog(
        &ctx,
        second,
        NewCatalog {
            movies: vec![heat(vec![
                NewMovieFile {
                    file_id: main,
                    role: Some(FileRole::Main),
                    expression: Some(0),
                    manifestation: Some(0),
                    part: None,
                },
                NewMovieFile {
                    file_id: trailer,
                    role: Some(FileRole::Extra),
                    expression: None,
                    manifestation: None,
                    part: None,
                },
            ])],
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(
        movie_files(&ctx, first).await,
        vec![(PathBuf::from("heat.mkv"), Some(FileRole::Main))],
    );
    assert_eq!(
        movie_files(&ctx, second).await,
        vec![
            (PathBuf::from("heat-trailer.mkv"), Some(FileRole::Extra)),
            (PathBuf::from("heat.mkv"), Some(FileRole::Main)),
        ],
    );
}

fn main_file(file_id: FileMeasurementRowId) -> Vec<NewMovieFile> {
    vec![NewMovieFile {
        file_id,
        role: Some(FileRole::Main),
        expression: Some(0),
        manifestation: Some(0),
        part: None,
    }]
}

async fn listed_release_year(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
) -> Option<i64> {
    let movies = list_movies(ctx, interpretation_id, &MovieSearchQuery::default())
        .await
        .unwrap()
        .movies;
    assert_eq!(movies.len(), 1);
    movies[0].release_year
}

/// The knowledge base's release date is the best source of a movie's year;
/// the claimed year is the fallback when the knowledge base has no date.
#[tokio::test]
async fn release_year_comes_from_the_kb_date_when_there_is_one() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &["heat.mkv"]).await];
    let from_kb = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let from_claim = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;

    record_catalog(
        &ctx,
        from_kb,
        NewCatalog {
            movies: vec![NewMovie {
                release_year: None,
                kb_data: MovieKbData {
                    release_date: Some(Date {
                        year: 1995,
                        month: 12,
                        day: 15,
                    }),
                    ..Default::default()
                },
                ..heat(main_file(file_id(&ctx, from_kb, "heat.mkv").await))
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    record_catalog(
        &ctx,
        from_claim,
        NewCatalog {
            movies: vec![NewMovie {
                release_year: Some(1996),
                kb_data: MovieKbData::default(),
                ..heat(main_file(file_id(&ctx, from_claim, "heat.mkv").await))
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(listed_release_year(&ctx, from_kb).await, Some(1995));
    assert_eq!(listed_release_year(&ctx, from_claim).await, Some(1996));
}

#[tokio::test]
async fn rejects_files_outside_the_ingest_set() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let ours = create_interpretation(
        &ctx,
        "test",
        &[ingest_files(&ctx, "ours", &["heat.mkv"]).await],
    )
    .await
    .unwrap()
    .row_id;
    let theirs = create_interpretation(
        &ctx,
        "test",
        &[ingest_files(&ctx, "theirs", &["heat.mkv"]).await],
    )
    .await
    .unwrap()
    .row_id;
    let their_file = file_id(&ctx, theirs, "heat.mkv").await;

    let err = record_catalog(
        &ctx,
        ours,
        NewCatalog {
            movies: vec![heat(main_file(their_file))],
            ..Default::default()
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(
        err.current_context(),
        CoreErrorContext::InvalidCatalog(CatalogError::FileNotInInterpretation(file)) if file == "theirs/heat.mkv"
    ));
}

#[tokio::test]
async fn rejects_recording_a_catalog_twice() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &["heat.mkv"]).await];
    let interpretation = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let main = file_id(&ctx, interpretation, "heat.mkv").await;
    let catalog = || NewCatalog {
        movies: vec![heat(main_file(main))],
        ..Default::default()
    };

    record_catalog(&ctx, interpretation, catalog())
        .await
        .unwrap();
    let err = record_catalog(&ctx, interpretation, catalog())
        .await
        .unwrap_err();

    assert!(matches!(
        err.current_context(),
        CoreErrorContext::InvalidCatalog(CatalogError::AlreadyRecorded(id)) if *id == interpretation
    ));
}

#[tokio::test]
async fn copies_of_a_movie_keep_their_main_files() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(
        &ctx,
        "media",
        &["heat.mkv", "heat-manifestation.mkv", "heat-unknown.mkv"],
    )
    .await];
    let interpretation = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let file = |rel_path| file_id(&ctx, interpretation, rel_path);
    let files = vec![
        NewMovieFile {
            file_id: file("heat.mkv").await,
            role: Some(FileRole::Main),
            expression: Some(0),
            manifestation: Some(0),
            part: None,
        },
        NewMovieFile {
            file_id: file("heat-manifestation.mkv").await,
            role: Some(FileRole::Main),
            expression: Some(0),
            manifestation: Some(1),
            part: None,
        },
        NewMovieFile {
            file_id: file("heat-unknown.mkv").await,
            role: None,
            expression: None,
            manifestation: None,
            part: None,
        },
    ];

    record_catalog(
        &ctx,
        interpretation,
        NewCatalog {
            movies: vec![heat(files)],
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(
        movie_files(&ctx, interpretation).await,
        vec![
            (
                PathBuf::from("heat-manifestation.mkv"),
                Some(FileRole::Main)
            ),
            (PathBuf::from("heat-unknown.mkv"), None),
            (PathBuf::from("heat.mkv"), Some(FileRole::Main)),
        ],
    );
}

#[tokio::test]
async fn rejects_movies_that_share_an_id() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &["heat.mkv", "heat-manifestation.mkv"]).await];
    let interpretation = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let first = file_id(&ctx, interpretation, "heat.mkv").await;
    let second = file_id(&ctx, interpretation, "heat-manifestation.mkv").await;

    // Grouping files into movies is up to the interpretation.
    let err = record_catalog(
        &ctx,
        interpretation,
        NewCatalog {
            movies: vec![heat(main_file(first)), heat(main_file(second))],
            ..Default::default()
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(
        err.current_context(),
        CoreErrorContext::InvalidCatalog(CatalogError::SharedMovieId { .. })
    ));
    // Names the files, as titles often don't match what's on disk.
    assert_eq!(
        err.to_string(),
        "movies \"Heat\" and \"Heat\" share TMDB ID 949, so they should be one movie. \
         Their files: media/heat.mkv, media/heat-manifestation.mkv"
    );
    let movies = list_movies(&ctx, interpretation, &MovieSearchQuery::default())
        .await
        .unwrap()
        .movies;
    assert!(movies.is_empty(), "rejected catalog must not be persisted");
}

#[tokio::test]
async fn rejects_movies_whose_main_files_dont_each_realize_one_expression() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &["heat.mkv", "heat-trailer.mkv"]).await];
    let interpretation = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let main = file_id(&ctx, interpretation, "heat.mkv").await;
    let trailer = file_id(&ctx, interpretation, "heat-trailer.mkv").await;
    let unspecified = || NewMovieExpression { edition: None };
    let file = |file_id, role, expression| NewMovieFile {
        file_id,
        role: Some(role),
        expression,
        manifestation: (role == FileRole::Main).then_some(0),
        part: None,
    };
    let wrong = [
        // A main file that realizes none.
        (vec![unspecified()], vec![file(main, FileRole::Main, None)]),
        // An extra that realizes one.
        (
            vec![unspecified()],
            vec![
                file(main, FileRole::Main, Some(0)),
                file(trailer, FileRole::Extra, Some(0)),
            ],
        ),
        // Two expressions of one edition.
        (
            vec![unspecified(), unspecified()],
            vec![
                file(main, FileRole::Main, Some(0)),
                file(trailer, FileRole::Main, Some(1)),
            ],
        ),
        // An expression that nothing realizes.
        (
            vec![
                unspecified(),
                NewMovieExpression {
                    edition: Some("Extended".into()),
                },
            ],
            vec![file(main, FileRole::Main, Some(0))],
        ),
    ];

    for (expressions, files) in wrong {
        let err = record_catalog(
            &ctx,
            interpretation,
            NewCatalog {
                movies: vec![NewMovie {
                    expressions,
                    ..heat(files)
                }],
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
        assert!(
            matches!(
                err.current_context(),
                CoreErrorContext::InvalidCatalog(CatalogError::InvalidMovieExpressions { .. })
            ),
            "{err}"
        );
    }
}

#[tokio::test]
async fn refuses_wrong_manifestations() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &["heat.cd1.mkv", "heat.cd2.mkv"]).await];
    let interpretation = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let cd1 = file_id(&ctx, interpretation, "heat.cd1.mkv").await;
    let cd2 = file_id(&ctx, interpretation, "heat.cd2.mkv").await;
    let file = |file_id, role, expression, manifestation, part| NewMovieFile {
        file_id,
        role: Some(role),
        expression,
        manifestation,
        part,
    };
    let wrong = [
        // A main file in no manifestation.
        (
            vec![NewMovieExpression { edition: None }],
            vec![file(cd1, FileRole::Main, Some(0), None, None)],
        ),
        // An extra in a manifestation.
        (
            vec![NewMovieExpression { edition: None }],
            vec![
                file(cd1, FileRole::Main, Some(0), Some(0), None),
                file(cd2, FileRole::Extra, None, Some(0), None),
            ],
        ),
        // An extra that's a part.
        (
            vec![NewMovieExpression { edition: None }],
            vec![
                file(cd1, FileRole::Main, Some(0), Some(0), None),
                file(cd2, FileRole::Extra, None, None, Some(2)),
            ],
        ),
        // One manifestation's parts realize different expressions.
        (
            vec![
                NewMovieExpression { edition: None },
                NewMovieExpression {
                    edition: Some("Extended".into()),
                },
            ],
            vec![
                file(cd1, FileRole::Main, Some(0), Some(0), Some(1)),
                file(cd2, FileRole::Main, Some(1), Some(0), Some(2)),
            ],
        ),
    ];

    for (expressions, files) in wrong {
        let err = record_catalog(
            &ctx,
            interpretation,
            NewCatalog {
                movies: vec![NewMovie {
                    expressions,
                    ..heat(files)
                }],
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
        assert!(
            matches!(
                err.current_context(),
                CoreErrorContext::InvalidCatalog(CatalogError::InvalidMovieManifestations { .. })
            ),
            "{err}"
        );
    }

    // Two parts of one manifestation, realizing one expression, are fine.
    record_catalog(
        &ctx,
        interpretation,
        NewCatalog {
            movies: vec![heat(vec![
                file(cd1, FileRole::Main, Some(0), Some(0), Some(1)),
                file(cd2, FileRole::Main, Some(0), Some(0), Some(2)),
            ])],
            ..Default::default()
        },
    )
    .await
    .unwrap();
}

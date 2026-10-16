mod common;

use common::{file_id, ingest_files};
use wanshitong_core::CoreContext;
use wanshitong_core::commands::{
    NewCatalog, NewMovie, NewMovieExpression, NewMovieFile, create_interpretation, record_catalog,
};
use wanshitong_core::domain::{FileRole, InterpretationRowId};
use wanshitong_core::queries::{MovieSearchQuery, list_movies};

/// Records one movie per title, each with its own file.
async fn catalog(ctx: &CoreContext, titles: &[&str]) -> InterpretationRowId {
    let paths: Vec<String> = titles.iter().map(|title| format!("{title}.mkv")).collect();
    let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    let sources = [ingest_files(ctx, "media", &paths).await];
    let interpretation = create_interpretation(ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let mut movies = Vec::new();
    for (i, (title, path)) in titles.iter().zip(&paths).enumerate() {
        movies.push(NewMovie {
            title: title.to_string(),
            expressions: vec![NewMovieExpression { edition: None }],
            tmdb_id: Some(i as u32 + 1),
            files: vec![NewMovieFile {
                file_id: file_id(ctx, interpretation, path).await,
                role: Some(FileRole::Main),
                expression: Some(0),
                manifestation: Some(0),
                part: None,
            }],
            ..Default::default()
        });
    }
    record_catalog(
        ctx,
        interpretation,
        NewCatalog {
            movies,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    interpretation
}

/// Lists every movie's title, a page of `limit` at a time.
async fn titles(
    ctx: &CoreContext,
    interpretation: InterpretationRowId,
    limit: usize,
) -> Vec<String> {
    let mut titles = Vec::new();
    let mut cursor = None;
    loop {
        let page = list_movies(
            ctx,
            interpretation,
            &MovieSearchQuery {
                cursor,
                limit,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        titles.extend(page.movies.into_iter().map(|movie| movie.title));
        cursor = page.next_cursor;
        if cursor.is_none() {
            return titles;
        }
    }
}

#[tokio::test]
async fn sorts_titles_without_a_leading_article() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let interpretation = catalog(
        &ctx,
        &[
            "The Thing",
            "Heat",
            "Theodora Goes Wild",
            "the Matrix",
            "Alien",
            "A Quiet Place",
            "An American Werewolf in London",
            "Annie Hall",
        ],
    )
    .await;

    let expected = [
        "Alien",
        "An American Werewolf in London",
        "Annie Hall",
        "Heat",
        "the Matrix",
        "A Quiet Place",
        "Theodora Goes Wild",
        "The Thing",
    ];
    assert_eq!(titles(&ctx, interpretation, 50).await, expected);
    // Pages continue from the previous one in the same order.
    assert_eq!(titles(&ctx, interpretation, 1).await, expected);
}

#[tokio::test]
async fn sorts_titles_by_their_words_and_numbers() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let interpretation = catalog(
        &ctx,
        &[
            "mother!",
            "Zodiac",
            "'Round Midnight",
            "...And Justice for All",
            "Léon: The Professional",
            "Lost in Translation",
            "12 Angry Men",
            "5 Centimeters per Second",
            "1917",
            "A.I. Artificial Intelligence",
            "Aliens",
            "Alien: Romulus",
            "Alien",
        ],
    )
    .await;

    let expected = [
        "5 Centimeters per Second",
        "12 Angry Men",
        "1917",
        "A.I. Artificial Intelligence",
        "Alien",
        "Alien: Romulus",
        "Aliens",
        "...And Justice for All",
        "Léon: The Professional",
        "Lost in Translation",
        "mother!",
        "'Round Midnight",
        "Zodiac",
    ];
    assert_eq!(titles(&ctx, interpretation, 50).await, expected);
}

#[tokio::test]
async fn lists_only_movies_with_extras() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let paths = [
        "Alien/Alien.mkv",
        "Alien/Making Of.mkv",
        "Brazil/Brazil.mkv",
        "Heat/Heat.mkv",
        "Heat/Heat.cut.mkv",
    ];
    let sources = [ingest_files(&ctx, "media", &paths).await];
    let interpretation = create_interpretation(&ctx, "test", &sources)
        .await
        .unwrap()
        .row_id;
    let file = async |path: &str, role: Option<FileRole>| NewMovieFile {
        file_id: file_id(&ctx, interpretation, path).await,
        role,
        expression: (role == Some(FileRole::Main)).then_some(0),
        manifestation: (role == Some(FileRole::Main)).then_some(0),
        part: None,
    };
    let movies = vec![
        NewMovie {
            title: "Alien".to_string(),
            expressions: vec![NewMovieExpression { edition: None }],
            tmdb_id: Some(348),
            files: vec![
                file("Alien/Alien.mkv", Some(FileRole::Main)).await,
                file("Alien/Making Of.mkv", Some(FileRole::Extra)).await,
            ],
            ..Default::default()
        },
        NewMovie {
            title: "Brazil".to_string(),
            expressions: vec![NewMovieExpression { edition: None }],
            tmdb_id: Some(68),
            files: vec![file("Brazil/Brazil.mkv", Some(FileRole::Main)).await],
            ..Default::default()
        },
        // A second file whose role is unknown is not an extra.
        NewMovie {
            title: "Heat".to_string(),
            expressions: vec![NewMovieExpression { edition: None }],
            tmdb_id: Some(949),
            files: vec![
                file("Heat/Heat.mkv", Some(FileRole::Main)).await,
                file("Heat/Heat.cut.mkv", None).await,
            ],
            ..Default::default()
        },
    ];
    record_catalog(
        &ctx,
        interpretation,
        NewCatalog {
            movies,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let page = list_movies(
        &ctx,
        interpretation,
        &MovieSearchQuery {
            with_extras: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let titles: Vec<_> = page.movies.into_iter().map(|movie| movie.title).collect();
    assert_eq!(titles, ["Alien"]);
}

mod common;

use common::wait_for_interpretations;
use std::path::PathBuf;
use tempfile::tempdir;
use uuid::Uuid;
use wanshitong_app::commands::{IngestStart, assert_claim, create_data_source, start_ingest};
use wanshitong_app::domain::PathId;
use wanshitong_app::domain::{
    ClaimPredicate, DataSourceConfig, DataSourceUuid, ExternalId, FileSystemSource, MovieUuid,
};
use wanshitong_app::queries::{
    MovieSearchQuery, Work, WorkEvidence, get_work_evidence, list_movies, wait_for_ingest,
};
use wanshitong_app::{AppContext, CoreContext};

/// Identifies each file as a movie with a title and TMDB ID, and returns the
/// movies by title. No knowledge base is asked about the IDs.
async fn identify(
    ctx: &AppContext,
    source: DataSourceUuid,
    movies: &[(&str, &str, u32)],
) -> Vec<(String, MovieUuid)> {
    for &(file, title, tmdb_id) in movies {
        for claim in [
            ClaimPredicate::MovieTitle(title.into()),
            ClaimPredicate::MovieExternalId(ExternalId::Tmdb(tmdb_id)),
        ] {
            assert_claim(ctx, &[PathId::new(source, file)], claim)
                .await
                .unwrap();
            wait_for_interpretations(ctx).await;
        }
    }
    list_movies(ctx, &MovieSearchQuery::default())
        .await
        .unwrap()
        .movies
        .into_iter()
        .map(|movie| (movie.title, movie.uuid))
        .collect()
}

#[tokio::test]
async fn evidence_is_a_works_files_and_the_assertions_about_them() {
    let tmp = tempdir().unwrap();
    for name in ["a.mkv", "b.mkv"] {
        tokio::fs::write(tmp.path().join(name), "not really a movie")
            .await
            .unwrap();
    }
    let core = CoreContext::new_in_memory().await.unwrap();
    let ctx = AppContext::new(core, Default::default(), "/fake/data");
    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: tmp.path().into(),
        follow_symlinks: false,
    });
    let source = create_data_source(&ctx, "media", config).await.unwrap();
    let IngestStart::Started(ingest) = start_ingest(&ctx, source.uuid).await.unwrap() else {
        panic!("no ingest of it was running");
    };
    wait_for_ingest(&ctx, ingest).await.unwrap();
    wait_for_interpretations(&ctx).await;
    let movies = identify(
        &ctx,
        source.uuid,
        &[("a.mkv", "Alpha", 1), ("b.mkv", "Beta", 2)],
    )
    .await;
    let alpha = movies.iter().find(|(title, _)| title == "Alpha").unwrap().1;

    let WorkEvidence {
        interpretation,
        work,
        files,
        top_directories: _,
        assertions,
    } = get_work_evidence(&ctx, alpha.0).await.unwrap().unwrap();
    assert!(interpretation.interpretation.is_canonical);
    assert!(matches!(work, Work::Movie(movie) if movie.uuid == alpha));
    let paths: Vec<_> = files.iter().map(|file| file.rel_path.clone()).collect();
    assert_eq!(paths, [PathBuf::from("a.mkv")]);
    assert_eq!(files[0].movies[0].uuid, alpha);
    let audit_log = files[0].audit_log.as_ref().unwrap();
    assert!(
        audit_log
            .claims
            .iter()
            .any(|claim| claim.pred == ClaimPredicate::MovieTitle("Alpha".into()))
    );
    // Only the assertions about Alpha's file, not Beta's, newest first.
    let claims: Vec<_> = assertions.into_iter().map(|a| a.claim).collect();
    assert_eq!(
        claims,
        [
            ClaimPredicate::MovieExternalId(ExternalId::Tmdb(1)),
            ClaimPredicate::MovieTitle("Alpha".into()),
        ]
    );

    assert!(
        get_work_evidence(&ctx, Uuid::nil())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn releases_are_the_top_directories_of_a_works_files() {
    let tmp = tempdir().unwrap();
    for path in ["Film/film.mkv", "Film/Extras/notes.txt", "loose.mkv"] {
        let path = tmp.path().join(path);
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(path, "not really a movie").await.unwrap();
    }
    let core = CoreContext::new_in_memory().await.unwrap();
    let ctx = AppContext::new(core, Default::default(), "/fake/data");
    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: tmp.path().into(),
        follow_symlinks: false,
    });
    let source = create_data_source(&ctx, "media", config).await.unwrap();
    let IngestStart::Started(ingest) = start_ingest(&ctx, source.uuid).await.unwrap() else {
        panic!("no ingest of it was running");
    };
    wait_for_ingest(&ctx, ingest).await.unwrap();
    wait_for_interpretations(&ctx).await;
    let movies = identify(
        &ctx,
        source.uuid,
        &[("Film/film.mkv", "Film", 1), ("loose.mkv", "Loose", 2)],
    )
    .await;
    let evidence = |title: &'static str| {
        let uuid = movies.iter().find(|(t, _)| t == title).unwrap().1;
        let ctx = ctx.clone();
        async move { get_work_evidence(&ctx, uuid.0).await.unwrap().unwrap() }
    };

    let film = evidence("Film").await;
    let [top_directory] = &film.top_directories[..] else {
        panic!("one top_directory");
    };
    assert_eq!(top_directory.data_source_uuid, source.uuid);
    let root = &top_directory.root;
    assert_eq!(root.name, "Film");
    assert!(root.has_work_files);
    let names: Vec<_> = root.files.iter().map(|f| f.rel_path.clone()).collect();
    assert_eq!(names, [PathBuf::from("Film/film.mkv")]);
    assert_eq!(root.files[0].movies[0].title, "Film");
    // Files the work doesn't have are in the tree too.
    let [extras] = &root.directories[..] else {
        panic!("one subdirectory");
    };
    assert_eq!(extras.name, "Extras");
    assert!(!extras.has_work_files);
    assert_eq!(extras.files.len(), 1);
    assert!(extras.files[0].movies.is_empty());

    // A file at a data source's root is in no top directory.
    assert!(evidence("Loose").await.top_directories.is_empty());
}

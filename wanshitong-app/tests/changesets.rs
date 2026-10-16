mod common;

use common::wait_for_interpretations;
use std::path::PathBuf;
use tempfile::tempdir;
use wanshitong_app::commands::{
    AssertionImport, ImportedAssertion, ImportedFile, IngestStart, NewAssertion, NewChangeset,
    NewMark, accept_changeset, add_assertions, assert_claim, correct_in_draft, create_changeset,
    create_data_source, discard_changeset, find_draft, import_assertions, interpret_changeset,
    mark_files, start_ingest,
};
use wanshitong_app::domain::PathId;
use wanshitong_app::domain::{
    ChangesetState, ChangesetUuid, ClaimPredicate, DataSourceConfig, DataSourceUuid, ExternalId,
    FileSystemSource,
};
use wanshitong_app::interpretations::InterpretationState;
use wanshitong_app::queries::{
    MovieSearchQuery, get_changeset, get_file, get_interpretation_or_canonical,
    list_changeset_interpretations, list_marks, list_movies, wait_for_ingest,
};
use wanshitong_app::{AppContext, CoreContext};

/// A library of files that nothing identifies without assertions.
async fn library(files: &[&str]) -> (AppContext, DataSourceUuid, tempfile::TempDir) {
    let tmp = tempdir().unwrap();
    for name in files {
        let path = tmp.path().join(name);
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
    (ctx, source.uuid, tmp)
}

/// Identifies files as a movie with a title and TMDB ID.
fn movie_assertions(
    files: &[(DataSourceUuid, &str)],
    title: &str,
    tmdb_id: u32,
) -> Vec<NewAssertion> {
    [
        ClaimPredicate::MovieTitle(title.into()),
        ClaimPredicate::MovieExternalId(ExternalId::Tmdb(tmdb_id)),
    ]
    .into_iter()
    .map(|claim| NewAssertion {
        files: files
            .iter()
            .map(|&(source, file)| PathId::new(source, file))
            .collect(),
        claim,
        rationale: Some("In the test".into()),
        confidence: None,
    })
    .collect()
}

async fn open(ctx: &AppContext) -> ChangesetUuid {
    create_changeset(
        ctx,
        NewChangeset {
            author: "agent".into(),
            description: "Identify the movies".into(),
        },
    )
    .await
    .unwrap()
}

async fn canonical_movies(ctx: &AppContext) -> Vec<String> {
    let mut titles: Vec<_> = list_movies(ctx, &MovieSearchQuery::default())
        .await
        .unwrap()
        .movies
        .into_iter()
        .map(|movie| movie.title)
        .collect();
    titles.sort();
    titles
}

#[tokio::test]
async fn a_changeset_is_interpreted_on_demand_and_accepted_as_it_was_previewed() {
    let (ctx, source, _tmp) = library(&["heat.mkv", "redline.mkv"]).await;
    let changeset = open(&ctx).await;
    let mut assertions = movie_assertions(&[(source, "heat.mkv")], "Heat", 949);
    assertions.extend(movie_assertions(
        &[(source, "redline.mkv")],
        "Redline",
        71883,
    ));
    add_assertions(&ctx, changeset, assertions).await.unwrap();

    // Nothing is interpreted until asked for, and the catalog is unchanged.
    assert!(canonical_movies(&ctx).await.is_empty());
    let preview = interpret_changeset(&ctx, changeset).await.unwrap();
    assert_eq!(preview.state, InterpretationState::Succeeded);
    assert!(!preview.interpretation.is_canonical);
    assert_eq!(
        preview.interpretation.name,
        "Changeset: Identify the movies"
    );
    assert_eq!(preview.interpretation.movie_count, 2);
    let of = preview.interpretation.changeset.as_ref().unwrap();
    assert_eq!((of.uuid, of.state), (changeset, ChangesetState::Open));
    // Asked again, with nothing changed, it's the same interpretation.
    let again = interpret_changeset(&ctx, changeset).await.unwrap();
    assert_eq!(again.interpretation.uuid, preview.interpretation.uuid);

    // Accepting makes the previewed interpretation canonical.
    let accepted = accept_changeset(&ctx, changeset).await.unwrap();
    assert_eq!(accepted, preview.interpretation.uuid);
    let canonical = get_interpretation_or_canonical(&ctx, None).await.unwrap();
    assert_eq!(canonical.interpretation.uuid, accepted);
    assert_eq!(canonical_movies(&ctx).await, ["Heat", "Redline"]);
}

#[tokio::test]
async fn a_changeset_accepted_after_others_is_interpreted_again() {
    let (ctx, source, _tmp) = library(&["heat.mkv", "redline.mkv"]).await;
    let changeset = open(&ctx).await;
    add_assertions(
        &ctx,
        changeset,
        movie_assertions(&[(source, "heat.mkv")], "Heat", 949),
    )
    .await
    .unwrap();
    let preview = interpret_changeset(&ctx, changeset).await.unwrap();

    // Meanwhile, corrections in the web UI.
    for claim in [
        ClaimPredicate::MovieTitle("Redline".into()),
        ClaimPredicate::MovieExternalId(ExternalId::Tmdb(71883)),
    ] {
        assert_claim(&ctx, &[PathId::new(source, "redline.mkv")], claim)
            .await
            .unwrap();
        wait_for_interpretations(&ctx).await;
    }

    let accepted = accept_changeset(&ctx, changeset).await.unwrap();
    assert_ne!(accepted, preview.interpretation.uuid);
    wait_for_interpretations(&ctx).await;
    let canonical = get_interpretation_or_canonical(&ctx, None).await.unwrap();
    assert_eq!(canonical.interpretation.uuid, accepted);
    assert_eq!(canonical.interpretation.name, "Identify the movies");
    assert_eq!(canonical.interpretation.movie_count, 2);
    // Made for the changeset, so it's known as what accepting it made.
    let of = canonical.interpretation.changeset.as_ref().unwrap();
    assert_eq!((of.uuid, of.state), (changeset, ChangesetState::Accepted));
    assert_eq!(
        list_changeset_interpretations(&ctx, changeset)
            .await
            .unwrap()
            .iter()
            .map(|details| details.interpretation.uuid)
            .collect::<Vec<_>>(),
        [accepted, preview.interpretation.uuid]
    );
}

#[tokio::test]
async fn a_discarded_changeset_leaves_the_catalog_alone() {
    let (ctx, source, _tmp) = library(&["heat.mkv"]).await;
    let before = get_interpretation_or_canonical(&ctx, None).await.unwrap();
    let changeset = open(&ctx).await;
    add_assertions(
        &ctx,
        changeset,
        movie_assertions(&[(source, "heat.mkv")], "Heat", 949),
    )
    .await
    .unwrap();
    interpret_changeset(&ctx, changeset).await.unwrap();
    discard_changeset(&ctx, changeset).await.unwrap();

    let after = get_interpretation_or_canonical(&ctx, None).await.unwrap();
    assert_eq!(after.interpretation.uuid, before.interpretation.uuid);
    assert!(canonical_movies(&ctx).await.is_empty());
}

#[tokio::test]
async fn correcting_in_the_draft_takes_back_what_it_contradicts() {
    let (ctx, source, _tmp) = library(&["treat.mkv", "other.mkv"]).await;
    let treat = || vec![PathId::new(source, "treat.mkv")];
    // Taken for a movie, by assertions: one about this file alone, one about
    // it and another file.
    let movie = open(&ctx).await;
    let mut assertions = movie_assertions(&[(source, "treat.mkv")], "New Year Treat", 1);
    assertions.push(NewAssertion {
        files: vec![
            PathId::new(source, "treat.mkv"),
            PathId::new(source, "other.mkv"),
        ],
        claim: ClaimPredicate::MovieReleaseYear(2024),
        rationale: None,
        confidence: None,
    });
    add_assertions(&ctx, movie, assertions).await.unwrap();
    accept_changeset(&ctx, movie).await.unwrap();
    wait_for_interpretations(&ctx).await;

    // Corrected to a special of a series.
    correct_in_draft(
        &ctx,
        &treat(),
        vec![
            ClaimPredicate::TvSeriesExternalId(ExternalId::Tmdb(63404)),
            ClaimPredicate::TvSeriesName("Taskmaster".into()),
            ClaimPredicate::Special,
        ],
    )
    .await
    .unwrap();
    let draft = find_draft(&ctx).await.unwrap().unwrap();
    let changeset = get_changeset(&ctx, draft).await.unwrap().unwrap();
    assert_eq!(changeset.assertions.len(), 3);
    // The movie's ID and title are retracted, but not the year, which is
    // about another file too.
    assert_eq!(changeset.retractions.len(), 2);

    // Corrected again, the draft's own contradicting assertions are
    // withdrawn, but not those that still agree, e.g. it's a special.
    correct_in_draft(
        &ctx,
        &treat(),
        vec![ClaimPredicate::TvSeriesExternalId(ExternalId::Tmdb(1))],
    )
    .await
    .unwrap();
    let changeset = get_changeset(&ctx, draft).await.unwrap().unwrap();
    let in_effect: Vec<_> = changeset
        .assertions
        .iter()
        .filter(|a| a.retracted_at().is_none())
        .map(|a| a.claim.clone())
        .collect();
    assert_eq!(
        in_effect,
        [
            ClaimPredicate::TvSeriesExternalId(ExternalId::Tmdb(1)),
            ClaimPredicate::Special,
            ClaimPredicate::TvSeriesName("Taskmaster".into()),
        ]
    );

    accept_changeset(&ctx, draft).await.unwrap();
    wait_for_interpretations(&ctx).await;
    assert!(find_draft(&ctx).await.unwrap().is_none());
    let file = get_file(&ctx, source, "treat.mkv".as_ref())
        .await
        .unwrap()
        .unwrap();
    assert!(file.movies.is_empty());
}

#[tokio::test]
async fn marking_one_of_a_discs_files_marks_the_disc() {
    let (ctx, source, _tmp) = library(&[
        "Film/BDMV/index.bdmv",
        "Film/BDMV/STREAM/00001.m2ts",
        "Film/BDMV/STREAM/00002.m2ts",
    ])
    .await;
    mark_files(
        &ctx,
        NewMark {
            label: "Film's streams".into(),
            note: String::new(),
            files: vec![
                PathId::new(source, "Film/BDMV/STREAM/00001.m2ts"),
                PathId::new(source, "Film/BDMV/STREAM/00002.m2ts"),
            ],
        },
    )
    .await
    .unwrap();
    let marks = list_marks(&ctx).await.unwrap();
    let files: Vec<_> = marks[0].files.iter().map(|f| f.rel_path.clone()).collect();
    assert_eq!(files, [PathBuf::from("Film/BDMV/index.bdmv")]);
}

/// An import that the core refuses, here an assertion about no files, leaves
/// no changeset behind, so that nothing waits for review by mistake.
#[tokio::test]
async fn a_refused_import_leaves_no_changeset() {
    let (ctx, source, _tmp) = library(&["a.mkv"]).await;
    let imported = |files: Vec<ImportedFile>| ImportedAssertion {
        files,
        claim: ClaimPredicate::Excluded,
        rationale: None,
        confidence: None,
        created_at: jiff::Timestamp::UNIX_EPOCH,
    };
    let file = || ImportedFile {
        data_source_uuid: DataSourceUuid(uuid::Uuid::nil()),
        data_source_name: "media".into(),
        rel_path: "a.mkv".into(),
    };
    let import = |assertions| AssertionImport {
        author: "import".into(),
        description: String::new(),
        assertions,
    };

    let error = import_assertions(&ctx, import(vec![imported(vec![file()]), imported(vec![])]))
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "core failed");
    assert!(
        wanshitong_app::queries::list_changesets(&ctx, Some(ChangesetState::Open))
            .await
            .unwrap()
            .is_empty()
    );

    let made = import_assertions(&ctx, import(vec![imported(vec![file()])]))
        .await
        .unwrap();
    assert_eq!(made.assertions.len(), 1);
    let changeset = get_changeset(&ctx, made.changeset).await.unwrap().unwrap();
    assert_eq!(changeset.state, ChangesetState::Open);
    assert_eq!(changeset.assertions[0].files[0].data_source_uuid, source);
}

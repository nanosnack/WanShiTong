mod common;

use common::{file_id, ingest_files};
use jiff::{SignedDuration, Timestamp};
use wanshitong_core::commands::{
    NewCatalog, NewChangeset, NewMovie, NewMovieExpression, NewMovieFile, create_changeset,
    create_changeset_interpretation, create_interpretation, finish_ingest, finish_interpretation,
    prune_interpretations, record_catalog, rename_interpretation, set_canonical_interpretation,
    start_ingest,
};
use wanshitong_core::domain::{FileRole, IngestStats, InterpretationOutcome};
use wanshitong_core::queries::{
    MovieSearchQuery, get_interpretation, list_interpretations, list_latest_successful_ingests,
    list_movies,
};
use wanshitong_core::{CoreContext, CoreErrorContext, NameError};

#[tokio::test]
async fn tracks_status_catalog_size_and_which_is_canonical() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &["heat.mkv"]).await];
    let good = create_interpretation(&ctx, "good", &sources).await.unwrap();
    let bad = create_interpretation(&ctx, "bad", &sources).await.unwrap();

    let status = |uuid| {
        let ctx = &ctx;
        async move {
            get_interpretation(ctx, uuid)
                .await
                .unwrap()
                .unwrap()
                .outcome()
        }
    };
    assert_eq!(status(good.uuid).await, InterpretationOutcome::Pending);

    let file_id = file_id(&ctx, good.row_id, "heat.mkv").await;
    record_catalog(
        &ctx,
        good.row_id,
        NewCatalog {
            movies: vec![NewMovie {
                title: "Heat".into(),
                expressions: vec![NewMovieExpression { edition: None }],
                files: vec![NewMovieFile {
                    file_id,
                    role: Some(FileRole::Main),
                    expression: Some(0),
                    manifestation: Some(0),
                    part: None,
                }],
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    finish_interpretation(&ctx, good.row_id, None)
        .await
        .unwrap();
    finish_interpretation(&ctx, bad.row_id, Some("TMDB is down"))
        .await
        .unwrap();
    set_canonical_interpretation(&ctx, good.row_id)
        .await
        .unwrap();

    // Newest first.
    let listed: Vec<_> = list_interpretations(&ctx)
        .await
        .unwrap()
        .into_iter()
        .map(|i| {
            (
                i.name.clone(),
                i.outcome(),
                i.error.clone(),
                i.is_canonical,
                i.movie_count,
                i.ingests.len(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        vec![
            (
                "bad".into(),
                InterpretationOutcome::Failed,
                Some("TMDB is down".into()),
                false,
                0,
                1
            ),
            (
                "good".into(),
                InterpretationOutcome::Success,
                None,
                true,
                1,
                1
            ),
        ]
    );
}

#[tokio::test]
async fn renames_to_valid_names_only() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &["heat.mkv"]).await];
    let interpretation = create_interpretation(&ctx, "first try", &sources)
        .await
        .unwrap();

    rename_interpretation(&ctx, interpretation.uuid, "  with Redline fixed ")
        .await
        .unwrap();
    let err = rename_interpretation(&ctx, interpretation.uuid, "   ")
        .await
        .unwrap_err();
    assert!(matches!(
        err.current_context(),
        CoreErrorContext::InvalidInterpretationName(NameError::Empty)
    ));

    let name = get_interpretation(&ctx, interpretation.uuid)
        .await
        .unwrap()
        .unwrap()
        .name;
    assert_eq!(name, "with Redline fixed");
}

#[tokio::test]
async fn only_successful_ingests_are_interpreted() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let (source, succeeded) = ingest_files(&ctx, "media", &["heat.mkv"]).await;
    let running = start_ingest(&ctx, source).await.unwrap();
    let failed = start_ingest(&ctx, source).await.unwrap();
    let stats = IngestStats {
        files_seen: 0,
        files_probed: 0,
        files_recorded: 0,
        media_files_recorded: 0,
        files_failed: 0,
    };
    finish_ingest(&ctx, failed.row_id, false, stats)
        .await
        .unwrap();

    // Newer ingests that are running or failed are passed over...
    assert_eq!(
        list_latest_successful_ingests(&ctx).await.unwrap(),
        [(source, succeeded)]
    );
    // ...and can't be interpreted directly.
    for ingest in [running.uuid, failed.uuid] {
        assert!(matches!(
            create_interpretation(&ctx, "partial", &[(source, ingest)]).await.unwrap_err().current_context(),
            CoreErrorContext::IngestNotSucceeded(uuid) if *uuid == ingest
        ));
    }
}

#[tokio::test]
async fn prunes_old_interpretations_but_those_in_use() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &["heat.mkv"]).await];
    let old = create_interpretation(&ctx, "old", &sources).await.unwrap();
    let canonical = create_interpretation(&ctx, "canonical", &sources)
        .await
        .unwrap();
    create_interpretation(&ctx, "running", &sources)
        .await
        .unwrap();
    let draft = create_changeset(
        &ctx,
        NewChangeset {
            author: "user".into(),
            description: "draft".into(),
        },
    )
    .await
    .unwrap();
    let drafted = create_changeset_interpretation(&ctx, "drafted", &sources, draft)
        .await
        .unwrap();
    // A catalog, so that deleting the interpretation has something to cascade
    // through.
    let file_id = file_id(&ctx, old.row_id, "heat.mkv").await;
    record_catalog(
        &ctx,
        old.row_id,
        NewCatalog {
            movies: vec![NewMovie {
                title: "Heat".into(),
                expressions: vec![NewMovieExpression { edition: None }],
                files: vec![NewMovieFile {
                    file_id,
                    role: Some(FileRole::Main),
                    expression: Some(0),
                    manifestation: Some(0),
                    part: None,
                }],
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    for interpretation in [&old, &canonical, &drafted] {
        finish_interpretation(&ctx, interpretation.row_id, None)
            .await
            .unwrap();
    }
    set_canonical_interpretation(&ctx, canonical.row_id)
        .await
        .unwrap();

    // Nothing started before the cutoff, nothing goes.
    let long_ago = Timestamp::now() - SignedDuration::from_hours(1);
    assert_eq!(prune_interpretations(&ctx, long_ago).await.unwrap(), vec![]);

    let later = Timestamp::now() + SignedDuration::from_hours(1);
    assert_eq!(
        prune_interpretations(&ctx, later).await.unwrap(),
        vec![old.uuid]
    );
    assert!(get_interpretation(&ctx, old.uuid).await.unwrap().is_none());
    assert!(
        list_movies(&ctx, old.row_id, &MovieSearchQuery::default())
            .await
            .unwrap()
            .movies
            .is_empty()
    );
    let kept: Vec<_> = list_interpretations(&ctx)
        .await
        .unwrap()
        .into_iter()
        .map(|i| i.name)
        .collect();
    assert_eq!(kept, vec!["drafted", "running", "canonical"]);
}

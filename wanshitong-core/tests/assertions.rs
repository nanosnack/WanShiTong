use wanshitong_core::domain::PathId;
mod common;

use common::{accept_assertion, ingest_files};
use std::path::PathBuf;
use wanshitong_core::commands::{
    NewAssertion, NewChangeset, accept_changeset, create_assertion, create_assertions,
    create_changeset, create_changeset_interpretation, create_interpretation, discard_changeset,
    retract_assertion, set_canonical_interpretation,
};
use wanshitong_core::domain::{
    Assertion, ChangesetState, ChangesetUuid, ClaimPredicate, DataSourceUuid, ExternalId,
    IngestUuid,
};
use wanshitong_core::queries::{
    disc_of_file, find_current_changeset_interpretation, get_changeset, list_active_assertions,
    list_changesets, list_interpretation_assertions,
};
use wanshitong_core::{CoreContext, CoreErrorContext};

fn tmdb(id: u32) -> ClaimPredicate {
    ClaimPredicate::MovieExternalId(ExternalId::Tmdb(id))
}

fn assertion(
    data_source: DataSourceUuid,
    rel_paths: &[&str],
    claim: ClaimPredicate,
) -> NewAssertion {
    NewAssertion {
        files: rel_paths
            .iter()
            .map(|rel_path| PathId::new(data_source, *rel_path))
            .collect(),
        claim,
        rationale: None,
        confidence: None,
    }
}

async fn open(ctx: &CoreContext) -> ChangesetUuid {
    create_changeset(
        ctx,
        NewChangeset {
            author: "agent".into(),
            description: "Identify things".into(),
        },
    )
    .await
    .unwrap()
}

/// Each assertion's claim and the paths of its files.
fn summarize(assertions: &[Assertion]) -> Vec<(ClaimPredicate, Vec<String>)> {
    assertions
        .iter()
        .map(|a| {
            let paths = a
                .files
                .iter()
                .map(|f| f.rel_path.display().to_string())
                .collect();
            (a.claim.clone(), paths)
        })
        .collect()
}

/// The claims a new interpretation includes, of the changeset's if given.
async fn interpreted(
    ctx: &CoreContext,
    sources: &[(DataSourceUuid, IngestUuid)],
    changeset: Option<ChangesetUuid>,
) -> Vec<(ClaimPredicate, Vec<String>)> {
    let interpretation = match changeset {
        Some(changeset) => create_changeset_interpretation(ctx, "preview", sources, changeset)
            .await
            .unwrap(),
        None => create_interpretation(ctx, "canonical", sources)
            .await
            .unwrap(),
    };
    let mut claims = summarize(
        &list_interpretation_assertions(ctx, interpretation.row_id)
            .await
            .unwrap(),
    );
    claims.sort_by_key(|(claim, paths)| (format!("{claim:?}"), paths.clone()));
    claims
}

#[tokio::test]
async fn an_open_changesets_assertions_are_only_in_its_own_interpretations() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &["heat.mkv"]).await];
    let (source, _) = sources[0];
    let changeset = open(&ctx).await;
    create_assertion(&ctx, changeset, assertion(source, &["heat.mkv"], tmdb(949)))
        .await
        .unwrap();

    assert!(list_active_assertions(&ctx).await.unwrap().is_empty());
    assert!(interpreted(&ctx, &sources, None).await.is_empty());
    assert_eq!(
        interpreted(&ctx, &sources, Some(changeset)).await,
        [(tmdb(949), vec!["heat.mkv".into()])]
    );

    // Accepted, they're in every new interpretation.
    accept_changeset(&ctx, changeset).await.unwrap();
    assert_eq!(
        interpreted(&ctx, &sources, None).await,
        [(tmdb(949), vec!["heat.mkv".into()])]
    );
    // And the changeset can't change any more.
    let err = create_assertion(&ctx, changeset, assertion(source, &["heat.mkv"], tmdb(1)))
        .await
        .unwrap_err();
    assert!(matches!(
        err.current_context(),
        CoreErrorContext::ChangesetNotOpen(_)
    ));
}

#[tokio::test]
async fn a_changesets_interpretation_is_what_accepting_it_would_make() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &["heat.mkv", "redline.mkv"]).await];
    let (source, _) = sources[0];
    let both = accept_assertion(
        &ctx,
        assertion(source, &["heat.mkv", "redline.mkv"], tmdb(1)),
    )
    .await;
    let year = accept_assertion(
        &ctx,
        assertion(
            source,
            &["redline.mkv"],
            ClaimPredicate::MovieReleaseYear(2009),
        ),
    )
    .await;

    let changeset = open(&ctx).await;
    // Replaces the first for redline.mkv only...
    create_assertion(
        &ctx,
        changeset,
        assertion(source, &["redline.mkv"], tmdb(71883)),
    )
    .await
    .unwrap();
    // ...and retracts the year.
    retract_assertion(&ctx, changeset, year).await.unwrap();
    let expected = vec![
        (tmdb(1), vec!["heat.mkv".into()]),
        (tmdb(71883), vec!["redline.mkv".into()]),
    ];
    assert_eq!(interpreted(&ctx, &sources, Some(changeset)).await, expected);
    // Nothing changes until it's accepted.
    assert_eq!(
        summarize(&list_active_assertions(&ctx).await.unwrap()).len(),
        2
    );

    accept_changeset(&ctx, changeset).await.unwrap();
    assert_eq!(interpreted(&ctx, &sources, None).await, expected);
    let both = list_active_assertions(&ctx)
        .await
        .unwrap()
        .into_iter()
        .find(|a| a.uuid == both)
        .unwrap();
    assert_eq!(summarize(&[both])[0].1, vec!["heat.mkv".to_string()]);
}

#[tokio::test]
async fn a_discarded_changeset_changes_nothing() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &["heat.mkv"]).await];
    let (source, _) = sources[0];
    let heat = accept_assertion(&ctx, assertion(source, &["heat.mkv"], tmdb(949))).await;

    let changeset = open(&ctx).await;
    create_assertion(&ctx, changeset, assertion(source, &["heat.mkv"], tmdb(1)))
        .await
        .unwrap();
    retract_assertion(&ctx, changeset, heat).await.unwrap();
    discard_changeset(&ctx, changeset).await.unwrap();

    assert_eq!(
        interpreted(&ctx, &sources, None).await,
        [(tmdb(949), vec!["heat.mkv".into()])]
    );
    let discarded = get_changeset(&ctx, changeset).await.unwrap().unwrap();
    assert_eq!(discarded.state, ChangesetState::Discarded);
    assert_eq!(summarize(&discarded.assertions).len(), 1);
    assert_eq!(discarded.retractions, [heat]);
}

#[tokio::test]
async fn retracting_ones_own_assertion_withdraws_it() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &["heat.mkv"]).await];
    let (source, _) = sources[0];
    let changeset = open(&ctx).await;
    let mistake = create_assertion(&ctx, changeset, assertion(source, &["heat.mkv"], tmdb(1)))
        .await
        .unwrap();
    retract_assertion(&ctx, changeset, mistake).await.unwrap();
    assert!(
        interpreted(&ctx, &sources, Some(changeset))
            .await
            .is_empty()
    );

    // Kept as history, withdrawn.
    let summary = list_changesets(&ctx, Some(ChangesetState::Open))
        .await
        .unwrap();
    assert_eq!(summary[0].assertion_count, 0);
    let changeset = get_changeset(&ctx, changeset).await.unwrap().unwrap();
    assert!(changeset.assertions[0].retracted_at().is_some());
}

#[tokio::test]
async fn only_what_is_in_effect_or_ones_own_can_be_retracted() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let (source, _) = ingest_files(&ctx, "media", &["heat.mkv"]).await;
    let other = open(&ctx).await;
    let pending = create_assertion(&ctx, other, assertion(source, &["heat.mkv"], tmdb(1)))
        .await
        .unwrap();

    let changeset = open(&ctx).await;
    let err = retract_assertion(&ctx, changeset, pending)
        .await
        .unwrap_err();
    assert!(matches!(
        err.current_context(),
        CoreErrorContext::AssertionNotRetractable { .. }
    ));
}

#[tokio::test]
async fn a_changesets_interpretation_is_current_until_something_changes() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &["heat.mkv", "redline.mkv"]).await];
    let (source, _) = sources[0];
    let changeset = open(&ctx).await;
    create_assertion(&ctx, changeset, assertion(source, &["heat.mkv"], tmdb(949)))
        .await
        .unwrap();
    assert_eq!(
        find_current_changeset_interpretation(&ctx, changeset)
            .await
            .unwrap(),
        None
    );

    let preview = create_changeset_interpretation(&ctx, "preview", &sources, changeset)
        .await
        .unwrap();
    let current = || find_current_changeset_interpretation(&ctx, changeset);
    assert_eq!(current().await.unwrap(), Some(preview.uuid));

    // Accepting it leaves it current, as it's what accepting made.
    accept_changeset(&ctx, changeset).await.unwrap();
    assert_eq!(current().await.unwrap(), Some(preview.uuid));

    // Another accepted changeset makes it out of date.
    accept_assertion(&ctx, assertion(source, &["redline.mkv"], tmdb(71883))).await;
    assert_eq!(current().await.unwrap(), None);
}

#[tokio::test]
async fn rejects_bad_assertions_and_changesets() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let unknown = DataSourceUuid(uuid::Uuid::nil());
    let changeset = open(&ctx).await;

    let err = create_assertion(
        &ctx,
        changeset,
        assertion(unknown, &["redline.mkv"], tmdb(71883)),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err.current_context(),
        CoreErrorContext::DataSourceNotFound(_)
    ));

    let err = create_assertion(&ctx, changeset, assertion(unknown, &[], tmdb(71883)))
        .await
        .unwrap_err();
    assert!(matches!(
        err.current_context(),
        CoreErrorContext::AssertionWithoutFiles
    ));

    let (source, _) = ingest_files(&ctx, "media", &["heat.mkv"]).await;
    let err = create_assertion(
        &ctx,
        changeset,
        NewAssertion {
            confidence: Some(1.5),
            ..assertion(source, &["heat.mkv"], tmdb(949))
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err.current_context(),
        CoreErrorContext::InvalidConfidence(_)
    ));

    let err = create_changeset(
        &ctx,
        NewChangeset {
            author: " ".into(),
            description: String::new(),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err.current_context(),
        CoreErrorContext::InvalidChangesetAuthor(_)
    ));

    // Nothing was recorded.
    let changeset = get_changeset(&ctx, changeset).await.unwrap().unwrap();
    assert!(changeset.assertions.is_empty());
}

#[tokio::test]
async fn assertions_keep_their_rationale_and_confidence() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let (source, _) = ingest_files(&ctx, "media", &["heat.mkv"]).await;
    let changeset = open(&ctx).await;
    create_assertion(
        &ctx,
        changeset,
        NewAssertion {
            rationale: Some(" Runtime 170m matches TMDB 949 ".into()),
            confidence: Some(0.9),
            ..assertion(source, &["heat.mkv"], tmdb(949))
        },
    )
    .await
    .unwrap();

    let changeset = get_changeset(&ctx, changeset).await.unwrap().unwrap();
    assert_eq!(changeset.author, "agent");
    let assertion = &changeset.assertions[0];
    assert_eq!(
        assertion.rationale.as_deref(),
        Some("Runtime 170m matches TMDB 949")
    );
    assert_eq!(assertion.confidence, Some(0.9));
}

#[tokio::test]
async fn a_batch_of_assertions_is_made_whole_or_not_at_all() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let (source, _) = ingest_files(&ctx, "media", &["heat.mkv"]).await;
    let unknown = DataSourceUuid(uuid::Uuid::nil());
    let changeset = open(&ctx).await;

    let err = create_assertions(
        &ctx,
        changeset,
        vec![
            assertion(source, &["heat.mkv"], tmdb(949)),
            assertion(unknown, &["heat.mkv"], tmdb(949)),
        ],
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err.current_context(),
        CoreErrorContext::DataSourceNotFound(_)
    ));
    let changeset = get_changeset(&ctx, changeset).await.unwrap().unwrap();
    assert!(changeset.assertions.is_empty());
}

#[tokio::test]
async fn only_an_accepted_changesets_interpretation_can_be_canonical() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let sources = [ingest_files(&ctx, "media", &["heat.mkv"]).await];
    let (source, _) = sources[0];
    let preview = async |changeset| {
        create_changeset_interpretation(&ctx, "preview", &sources, changeset)
            .await
            .unwrap()
            .row_id
    };

    // Open, or discarded, its changes haven't happened.
    let open_changeset = open(&ctx).await;
    let heat = accept_assertion(&ctx, assertion(source, &["heat.mkv"], tmdb(949))).await;
    retract_assertion(&ctx, open_changeset, heat).await.unwrap();
    let open_preview = preview(open_changeset).await;
    let err = set_canonical_interpretation(&ctx, open_preview)
        .await
        .unwrap_err();
    assert!(matches!(
        err.current_context(),
        CoreErrorContext::InterpretationOfUnacceptedChangeset { .. }
    ));
    discard_changeset(&ctx, open_changeset).await.unwrap();
    assert!(
        set_canonical_interpretation(&ctx, open_preview)
            .await
            .is_err()
    );

    // Accepted, they have.
    let accepted = open(&ctx).await;
    create_assertion(&ctx, accepted, assertion(source, &["heat.mkv"], tmdb(1)))
        .await
        .unwrap();
    let accepted_preview = preview(accepted).await;
    accept_changeset(&ctx, accepted).await.unwrap();
    set_canonical_interpretation(&ctx, accepted_preview)
        .await
        .unwrap();

    // As can any interpretation of the accepted assertions.
    let plain = create_interpretation(&ctx, "plain", &sources)
        .await
        .unwrap();
    set_canonical_interpretation(&ctx, plain.row_id)
        .await
        .unwrap();
}

#[tokio::test]
async fn an_assertion_names_a_disc_by_its_index_file_not_its_other_files() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let (source, _) = ingest_files(
        &ctx,
        "media",
        &["Film/BDMV/index.bdmv", "Film/BDMV/STREAM/00001.m2ts"],
    )
    .await;
    let changeset = open(&ctx).await;

    let stream = create_assertions(
        &ctx,
        changeset,
        vec![assertion(source, &["Film/BDMV/STREAM/00001.m2ts"], tmdb(1))],
    )
    .await;
    assert!(matches!(
        stream.unwrap_err().current_context(),
        CoreErrorContext::PartOfDisc { index, .. } if index == "Film/BDMV/index.bdmv"
    ));
    assert_eq!(
        disc_of_file(&ctx, source, "Film/BDMV/STREAM/00001.m2ts".as_ref())
            .await
            .unwrap(),
        Some(PathBuf::from("Film/BDMV/index.bdmv"))
    );

    create_assertions(
        &ctx,
        changeset,
        vec![assertion(source, &["Film/BDMV/index.bdmv"], tmdb(1))],
    )
    .await
    .unwrap();
}

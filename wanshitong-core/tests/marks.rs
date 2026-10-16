mod common;

use common::{accept_assertion, ingest_files};
use wanshitong_core::commands::{
    NewAssertion, NewChangeset, NewMark, accept_changeset, create_assertion, create_changeset,
    create_mark, delete_mark, retract_assertion,
};
use wanshitong_core::domain::{
    ClaimPredicate, DataSourceUuid, ExternalId, MarkProgress, MarkUuid, PathId,
};
use wanshitong_core::queries::list_marks;
use wanshitong_core::{CoreContext, CoreErrorContext};

fn files(source: DataSourceUuid, paths: &[&str]) -> Vec<PathId> {
    paths
        .iter()
        .map(|path| PathId::new(source, *path))
        .collect()
}

fn tmdb(source: DataSourceUuid, paths: &[&str], id: u32) -> NewAssertion {
    NewAssertion {
        files: files(source, paths),
        claim: ClaimPredicate::MovieExternalId(ExternalId::Tmdb(id)),
        rationale: None,
        confidence: None,
    }
}

async fn progress(ctx: &CoreContext, uuid: MarkUuid) -> MarkProgress {
    let marks = list_marks(ctx).await.unwrap();
    marks.iter().find(|m| m.uuid == uuid).unwrap().progress
}

#[tokio::test]
async fn a_mark_is_pending_then_done_as_its_files_are_corrected() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let (source, _) = ingest_files(&ctx, "media", &["heat.mkv", "trailer.mkv"]).await;
    // An assertion from before the mark doesn't correct it.
    accept_assertion(&ctx, tmdb(source, &["heat.mkv"], 1)).await;
    let mark = create_mark(
        &ctx,
        NewMark {
            label: " movie Heat ".into(),
            note: "wrong movie".into(),
            files: files(source, &["heat.mkv", "trailer.mkv"]),
        },
    )
    .await
    .unwrap();
    let marks = list_marks(&ctx).await.unwrap();
    assert_eq!(marks[0].label, "movie Heat");
    assert_eq!(marks[0].note, "wrong movie");
    assert_eq!(marks[0].files.len(), 2);
    assert_eq!(marks[0].progress, MarkProgress::Open);

    // Asserting in an open changeset makes it pending...
    let changeset = create_changeset(
        &ctx,
        NewChangeset {
            author: "user".into(),
            description: String::new(),
        },
    )
    .await
    .unwrap();
    let mistake = create_assertion(&ctx, changeset, tmdb(source, &["heat.mkv"], 2))
        .await
        .unwrap();
    assert_eq!(progress(&ctx, mark).await, MarkProgress::Pending);
    // ...unless it's withdrawn.
    retract_assertion(&ctx, changeset, mistake).await.unwrap();
    assert_eq!(progress(&ctx, mark).await, MarkProgress::Open);

    // Accepted for some of its files, it's still pending; for all, done.
    create_assertion(&ctx, changeset, tmdb(source, &["heat.mkv"], 949))
        .await
        .unwrap();
    accept_changeset(&ctx, changeset).await.unwrap();
    assert_eq!(progress(&ctx, mark).await, MarkProgress::Open);
    accept_assertion(&ctx, tmdb(source, &["trailer.mkv"], 949)).await;
    assert_eq!(progress(&ctx, mark).await, MarkProgress::Done);
}

#[tokio::test]
async fn marks_can_be_removed() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let (source, _) = ingest_files(&ctx, "media", &["sample.mkv"]).await;
    let mark = create_mark(
        &ctx,
        NewMark {
            label: "sample.mkv".into(),
            note: String::new(),
            files: files(source, &["sample.mkv"]),
        },
    )
    .await
    .unwrap();

    delete_mark(&ctx, mark).await.unwrap();
    delete_mark(&ctx, mark).await.unwrap();
    assert!(list_marks(&ctx).await.unwrap().is_empty());
    assert!(matches!(
        create_mark(
            &ctx,
            NewMark {
                label: String::new(),
                note: String::new(),
                files: vec![],
            },
        )
        .await
        .unwrap_err()
        .current_context(),
        CoreErrorContext::MarkWithoutFiles
    ));
}

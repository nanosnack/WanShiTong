use wanshitong_core::domain::PathId;
mod common;

use common::{accept_assertion, ingest_files};
use wanshitong_core::commands::{
    NewAssertion, NewChangeset, create_assertion, create_changeset, create_data_source,
    create_interpretation, create_view, delete_data_source, delete_view, restore_data_source,
    start_ingest,
};
use wanshitong_core::domain::{
    ClaimPredicate, DataSourceConfig, DataSourceUuid, ExternalId, FileSystemSource, JellyfinView,
    ViewConfig,
};
use wanshitong_core::queries::{
    get_data_source, get_interpretation, list_data_sources, list_deleted_data_sources,
    list_interpretation_assertions,
};
use wanshitong_core::{CoreContext, CoreErrorContext};

fn heat_is_tmdb_1(data_source: DataSourceUuid) -> NewAssertion {
    NewAssertion {
        files: vec![PathId::new(data_source, "heat.mkv")],
        claim: ClaimPredicate::MovieExternalId(ExternalId::Tmdb(1)),
        rationale: None,
        confidence: None,
    }
}

fn fs(root_path: &str) -> DataSourceConfig {
    DataSourceConfig::FileSystem(FileSystemSource {
        root_path: root_path.into(),
        follow_symlinks: false,
    })
}

fn names(sources: &[wanshitong_core::domain::DataSource]) -> Vec<&str> {
    sources.iter().map(|s| s.name.as_str()).collect()
}

#[tokio::test]
async fn a_deleted_source_takes_no_part_in_new_work() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let kept = ingest_files(&ctx, "kept", &["heat.mkv"]).await;
    let gone = ingest_files(&ctx, "gone", &["heat.mkv"]).await;
    accept_assertion(&ctx, heat_is_tmdb_1(kept.0)).await;
    accept_assertion(&ctx, heat_is_tmdb_1(gone.0)).await;
    let before = create_interpretation(&ctx, "before", &[kept, gone])
        .await
        .unwrap();

    delete_data_source(&ctx, gone.0).await.unwrap();

    assert_eq!(names(&list_data_sources(&ctx).await.unwrap()), ["kept"]);
    assert_eq!(
        names(&list_deleted_data_sources(&ctx).await.unwrap()),
        ["gone"]
    );
    assert!(matches!(
        create_interpretation(&ctx, "after", &[kept, gone]).await.unwrap_err().current_context(),
        CoreErrorContext::DataSourceDeleted(uuid) if *uuid == gone.0
    ));
    assert!(matches!(
        start_ingest(&ctx, gone.0).await.unwrap_err().current_context(),
        CoreErrorContext::DataSourceDeleted(uuid) if *uuid == gone.0
    ));
    let changeset = create_changeset(
        &ctx,
        NewChangeset {
            author: "user".into(),
            description: String::new(),
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        create_assertion(&ctx, changeset, heat_is_tmdb_1(gone.0)).await.unwrap_err().current_context(),
        CoreErrorContext::DataSourceDeleted(uuid) if *uuid == gone.0
    ));

    // Its assertions stay active, but new interpretations leave them out.
    let after = create_interpretation(&ctx, "after", &[kept]).await.unwrap();
    let asserted_about = |assertions: Vec<wanshitong_core::domain::Assertion>| {
        assertions
            .into_iter()
            .map(|a| a.files[0].data_source_uuid)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        asserted_about(
            list_interpretation_assertions(&ctx, after.row_id)
                .await
                .unwrap()
        ),
        [kept.0]
    );

    // What came before still refers to it.
    assert_eq!(
        asserted_about(
            list_interpretation_assertions(&ctx, before.row_id)
                .await
                .unwrap()
        ),
        // Newest first.
        [gone.0, kept.0]
    );
    let before = get_interpretation(&ctx, before.uuid)
        .await
        .unwrap()
        .unwrap();
    assert!(
        before
            .ingests
            .iter()
            .any(|i| i.data_source_uuid == gone.0 && i.data_source_name == "gone")
    );
    let source = get_data_source(&ctx, gone.0).await.unwrap().unwrap();
    assert!(source.deleted_at.is_some());
    assert_eq!(source.latest_ingest, Some(gone.1));
}

#[tokio::test]
async fn restoring_brings_back_its_ingests_and_assertions() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let source = ingest_files(&ctx, "media", &["heat.mkv"]).await;
    accept_assertion(&ctx, heat_is_tmdb_1(source.0)).await;

    delete_data_source(&ctx, source.0).await.unwrap();
    // Deleting and restoring twice changes nothing more.
    delete_data_source(&ctx, source.0).await.unwrap();
    restore_data_source(&ctx, source.0).await.unwrap();
    restore_data_source(&ctx, source.0).await.unwrap();

    let restored = get_data_source(&ctx, source.0).await.unwrap().unwrap();
    assert_eq!(restored.deleted_at, None);
    assert_eq!(restored.latest_ingest, Some(source.1));
    let interpretation = create_interpretation(&ctx, "restored", &[source])
        .await
        .unwrap();
    assert_eq!(
        list_interpretation_assertions(&ctx, interpretation.row_id)
            .await
            .unwrap()
            .len(),
        1
    );
    start_ingest(&ctx, source.0).await.unwrap();
}

#[tokio::test]
async fn a_deleted_sources_name_and_place_can_be_taken() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    // `ingest_files` roots its data source at /media.
    let (old, _) = ingest_files(&ctx, "media", &[]).await;
    delete_data_source(&ctx, old).await.unwrap();

    // Its name, until it's restored.
    let new = create_data_source(&ctx, "media", fs("/elsewhere"))
        .await
        .unwrap();
    assert!(matches!(
        restore_data_source(&ctx, old).await.unwrap_err().current_context(),
        CoreErrorContext::DataSourceNameExists(name) if name == "media"
    ));
    delete_data_source(&ctx, new.uuid).await.unwrap();

    // Its place, by a view, until it's restored.
    let view = create_view(
        &ctx,
        "jellyfin",
        ViewConfig::Jellyfin(JellyfinView {
            root_path: "/media/view".into(),
        }),
        true,
    )
    .await
    .unwrap();
    assert!(matches!(
        restore_data_source(&ctx, old).await.unwrap_err().current_context(),
        CoreErrorContext::DataSourceOverlapsView { view, .. } if view == "jellyfin"
    ));
    delete_view(&ctx, view.uuid).await.unwrap();

    restore_data_source(&ctx, old).await.unwrap();
    assert_eq!(names(&list_data_sources(&ctx).await.unwrap()), ["media"]);
}

#[tokio::test]
async fn deleting_or_restoring_an_unknown_source_fails() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let uuid = DataSourceUuid(uuid::Uuid::new_v4());
    assert!(matches!(
        delete_data_source(&ctx, uuid)
            .await
            .unwrap_err()
            .current_context(),
        CoreErrorContext::DataSourceNotFound(_)
    ));
    assert!(matches!(
        restore_data_source(&ctx, uuid)
            .await
            .unwrap_err()
            .current_context(),
        CoreErrorContext::DataSourceNotFound(_)
    ));
}

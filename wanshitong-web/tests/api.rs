//! The JSON API's own behavior: pinning reads to an interpretation, keeping
//! evidence compact, and reporting errors. What the queries return is tested in
//! wanshitong-app.

use axum::routing::RouterIntoService;
use serde_json::json;
use std::{path::Path, process::Command, time::Duration};
use tempfile::tempdir;
use wanshitong_api::v1::{
    AssertedFile, Assertion, ChangesetState, ChangesetsQuery, Claim, DataSource, DiffQuery,
    Directory, ErrorCode, ExternalId, FileQuery, FileRef, HealthStatus, ImportAssertions,
    MarkProgress, MarksQuery, NewAssertion, NewAssertions, NewChangeset, NewRetraction,
    PlacementGroup, UnidentifiedQuery, UnidentifiedVideos, WorkChangeKind,
};
use wanshitong_app::commands::{
    IngestStart, NewMark, create_data_source, create_interpretation, mark_files, start_ingest,
};
use wanshitong_app::domain::PathId;
use wanshitong_app::domain::{DataSourceConfig, DataSourceUuid, FileSystemSource};
use wanshitong_app::interpretations::InterpretationState;
use wanshitong_app::queries::{list_interpretations, wait_for_ingest};
use wanshitong_client::{Client, Error, RequestBody};
use wanshitong_web::{AppContext, CoreContext, WebContext, router};

/// Polls until no interpretation is running.
async fn wait_for_interpretations(ctx: &AppContext) {
    for _ in 0..200 {
        let interpretations = list_interpretations(ctx).await.unwrap();
        if interpretations
            .iter()
            .all(|i| i.state != InterpretationState::Running)
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("interpretations still running");
}

/// A second of video, which nothing will identify without a knowledge base, or
/// one frame of it for an image, e.g. a `.jpg`, which ffprobe also reads as
/// a video stream.
fn write_video(path: &Path) {
    let status = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-f", "lavfi", "-i"])
        .arg("testsrc=duration=1:size=64x48:rate=5")
        .args([
            "-frames:v",
            if path.extension().is_some_and(|ext| ext == "jpg") {
                "1"
            } else {
                "5"
            },
        ])
        .arg(path)
        .status()
        .expect("ffmpeg runs");
    assert!(status.success(), "ffmpeg made {}", path.display());
}

/// An ingested and interpreted library of `files`. Those named `.mkv` are
/// videos, and `.jpg` images; the rest are text.
async fn library(files: &[&str]) -> (AppContext, DataSourceUuid, tempfile::TempDir) {
    let tmp = tempdir().unwrap();
    for path in files {
        let path = tmp.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        if path
            .extension()
            .is_some_and(|ext| ext == "mkv" || ext == "jpg")
        {
            write_video(&path);
        } else {
            std::fs::write(path, "not a video").unwrap();
        }
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

/// The `wst` CLI's client, handed the router itself instead of a connection
/// to it, so that the client's requests and its reading of the responses
/// are tested along with the endpoints.
fn client(ctx: &AppContext) -> Client<RouterIntoService<RequestBody>> {
    Client::new(
        router(WebContext::new(ctx.clone())).into_service(),
        "http://wst",
    )
}

/// The API's status and error from a request it refused.
fn refused(error: Error) -> (u16, ErrorCode, String) {
    match error {
        Error::Api { status, error } => (status.as_u16(), error.code, error.message),
        other => panic!("not the API's error: {other:?}"),
    }
}

fn agent() -> NewChangeset {
    NewChangeset {
        author: "agent".into(),
        description: String::new(),
    }
}

fn assertion(source: DataSourceUuid, path: &str, claim: Claim) -> NewAssertion {
    NewAssertion {
        files: vec![FileRef {
            data_source: source.0,
            path: path.into(),
        }],
        claim,
        rationale: None,
        confidence: None,
    }
}

fn paths(page: &UnidentifiedVideos) -> Vec<&str> {
    page.files.iter().map(|file| file.path.as_str()).collect()
}

#[tokio::test]
async fn pages_keep_reading_their_interpretation() {
    let (ctx, _, _tmp) = library(&["a.mkv", "b.mkv", "notes.txt"]).await;
    let client = client(&ctx);
    let one = |cursor: Option<String>, interpretation| UnidentifiedQuery {
        interpretation,
        name: None,
        cursor,
        limit: Some(1),
    };

    let first = client
        .get_unidentified_videos(&one(None, None))
        .await
        .unwrap();
    assert!(first.interpretation.canonical);
    assert_eq!(paths(&first), ["a.mkv"]);

    // A reinterpretation takes over the catalog...
    create_interpretation(&ctx, "Again", true).await.unwrap();
    wait_for_interpretations(&ctx).await;
    let canonical = client
        .get_unidentified_videos(&Default::default())
        .await
        .unwrap();
    assert_ne!(canonical.interpretation.uuid, first.interpretation.uuid);
    assert_eq!(paths(&canonical), ["a.mkv", "b.mkv"]);

    // ...but the next page is still the first page's interpretation.
    let cursor = first.next_cursor.unwrap();
    let second = client
        .get_unidentified_videos(&one(Some(cursor.clone()), None))
        .await
        .unwrap();
    assert_eq!(second.interpretation.uuid, first.interpretation.uuid);
    assert!(!second.interpretation.canonical);
    assert_eq!(paths(&second), ["b.mkv"]);
    assert!(second.next_cursor.is_none());

    // An interpretation can be read by name, too.
    let named = client
        .get_unidentified_videos(&one(None, Some(first.interpretation.uuid)))
        .await
        .unwrap();
    assert_eq!(named.interpretation.uuid, first.interpretation.uuid);

    // A cursor can't be used with another interpretation.
    let error = client
        .get_unidentified_videos(&one(Some(cursor), Some(canonical.interpretation.uuid)))
        .await
        .unwrap_err();
    let (status, code, _) = refused(error);
    assert_eq!((status, code), (400, ErrorCode::BadRequest));
}

#[tokio::test]
async fn releases_fold_what_doesnt_lead_to_the_file() {
    let (ctx, source, _tmp) = library(&[
        "Box/Film/film.txt",
        "Box/Film/Scans/1.jpg",
        "Box/Film/Scans/Inlay/2.jpg",
        "Box/Film/Scans/clip.mkv",
        "Box/CDs/Disc 1/track.flac",
        "Box/CDs/Disc 2/track.flac",
    ])
    .await;
    let client = client(&ctx);
    let names = |dirs: &[Directory]| -> Vec<(String, usize, usize)> {
        dirs.iter()
            .map(|dir| (dir.name.clone(), dir.file_count, dir.folded_directories))
            .collect()
    };
    let query = |directory_files| FileQuery {
        interpretation: None,
        data_source: source.0,
        path: "Box/Film/film.txt".into(),
        directory_files,
    };

    // The way to the file is expanded, and its neighbours fold everything
    // below them.
    let evidence = client.get_file_evidence(&query(false)).await.unwrap();
    assert_eq!(evidence.file.path, "Box/Film/film.txt");
    let box_dir = &evidence.top_directory.unwrap().root;
    assert_eq!(
        names(&box_dir.directories),
        [("CDs".into(), 2, 2), ("Film".into(), 1, 0)]
    );
    let film = &box_dir.directories[1];
    assert!(film.has_work_files);
    assert_eq!(names(&film.directories), [("Scans".into(), 3, 1)]);
    let scans = &film.directories[0];
    assert!(scans.directories.is_empty());
    // Only the real video counts as one; the scans are other files.
    let placements: Vec<_> = scans
        .placements
        .iter()
        .map(|count| (count.placement.clone(), count.files))
        .collect();
    assert_eq!(
        placements,
        [
            (PlacementGroup::Other, 2),
            (PlacementGroup::UnidentifiedVideos, 1)
        ]
    );

    // Unless every file is asked for.
    let evidence = client.get_file_evidence(&query(true)).await.unwrap();
    let box_dir = &evidence.top_directory.unwrap().root;
    assert_eq!(
        names(&box_dir.directories),
        [("CDs".into(), 0, 0), ("Film".into(), 1, 0)]
    );
    assert_eq!(box_dir.directories[0].directories.len(), 2);
}

/// Requests the client's methods wouldn't make, so they go as paths.
#[tokio::test]
async fn errors_are_json() {
    let core = CoreContext::new_in_memory().await.unwrap();
    let ctx = AppContext::new(core, Default::default(), "/fake/data");
    let client = client(&ctx);
    let nil = "00000000-0000-0000-0000-000000000000";

    let cases = [
        (format!("/works/{nil}"), 404, ErrorCode::NotFound),
        ("/works/not-a-uuid".into(), 400, ErrorCode::BadRequest),
        ("/unidentified".into(), 409, ErrorCode::NoCatalog),
        (
            format!("/unidentified?interpretation={nil}"),
            404,
            ErrorCode::NotFound,
        ),
        (
            "/unidentified?cursor=nonsense".into(),
            400,
            ErrorCode::BadRequest,
        ),
        (
            "/unidentified?limit=many".into(),
            400,
            ErrorCode::BadRequest,
        ),
        (
            "/files/evidence?path=a.mkv".into(),
            400,
            ErrorCode::BadRequest,
        ),
        ("/nothing-here".into(), 404, ErrorCode::NotFound),
        (
            "/kb/search?kind=movie&query=redline".into(),
            503,
            ErrorCode::NoKnowledgeBase,
        ),
        (
            "/kb/search?kind=book&query=x".into(),
            400,
            ErrorCode::BadRequest,
        ),
    ];
    for (path, expected_status, expected_code) in cases {
        let error = client
            .get::<serde_json::Value>(&path, &())
            .await
            .unwrap_err();
        let (status, code, _) = refused(error);
        assert_eq!((status, code), (expected_status, expected_code), "{path}");
    }

    // A file that the interpretation didn't see.
    let (ctx, source, _tmp) = library(&["a.mkv"]).await;
    let error = self::client(&ctx)
        .get_file_evidence(&FileQuery {
            interpretation: None,
            data_source: source.0,
            path: "b.mkv".into(),
            directory_files: false,
        })
        .await
        .unwrap_err();
    let (status, code, _) = refused(error);
    assert_eq!((status, code), (404, ErrorCode::NotFound));
}

#[tokio::test]
async fn assertions_in_a_changeset_apply_once_its_accepted() {
    let (ctx, source, _tmp) = library(&["a.mkv", "b.mkv"]).await;
    let client = client(&ctx);
    let changeset = client
        .create_changeset(&NewChangeset {
            author: "agent".into(),
            description: "Identify a".into(),
        })
        .await
        .unwrap();
    assert_eq!(changeset.state, ChangesetState::Open);

    let created = client
        .add_assertions(
            changeset.uuid,
            &NewAssertions {
                assertions: vec![NewAssertion {
                    rationale: Some("Runtime matches".into()),
                    confidence: Some(0.9),
                    ..assertion(
                        source,
                        "a.mkv",
                        Claim::MovieExternalId(ExternalId::Tmdb(949)),
                    )
                }],
            },
        )
        .await
        .unwrap();
    let pending = client.get_changeset(changeset.uuid).await.unwrap();
    assert_eq!(pending.assertions[0].uuid, created.assertions[0]);
    assert_eq!(
        pending.assertions[0].rationale.as_deref(),
        Some("Runtime matches")
    );
    // Pending, so a.mkv is still unidentified.
    let unidentified = client
        .get_unidentified_videos(&Default::default())
        .await
        .unwrap();
    assert_eq!(paths(&unidentified), ["a.mkv", "b.mkv"]);

    let accepted = client.accept_changeset(changeset.uuid).await.unwrap();
    wait_for_interpretations(&ctx).await;
    let unidentified = client
        .get_unidentified_videos(&Default::default())
        .await
        .unwrap();
    assert_eq!(unidentified.interpretation.uuid, accepted.interpretation);
    assert_eq!(paths(&unidentified), ["b.mkv"]);

    // An accepted changeset can't change.
    let error = client.discard_changeset(changeset.uuid).await.unwrap_err();
    let (status, code, _) = refused(error);
    assert_eq!((status, code), (409, ErrorCode::Conflict));
}

/// What `wst assertions export` writes, and the Assertions page's Export.
#[tokio::test]
async fn the_assertions_in_effect_are_listed() {
    let (ctx, source, _tmp) = library(&["a.mkv", "b.mkv"]).await;
    let client = client(&ctx);
    let changeset = client.create_changeset(&agent()).await.unwrap();
    let created = client
        .add_assertions(
            changeset.uuid,
            &NewAssertions {
                assertions: vec![
                    assertion(
                        source,
                        "a.mkv",
                        Claim::MovieExternalId(ExternalId::Tmdb(949)),
                    ),
                    NewAssertion {
                        rationale: Some("A sample".into()),
                        ..assertion(source, "b.mkv", Claim::Excluded)
                    },
                ],
            },
        )
        .await
        .unwrap();
    // Pending assertions aren't in effect.
    let active = client.list_assertions().await.unwrap();
    assert!(active.assertions.is_empty());

    client.accept_changeset(changeset.uuid).await.unwrap();
    let active = client.list_assertions().await.unwrap();
    let uuids: Vec<_> = active.assertions.iter().map(|a| a.uuid).collect();
    assert_eq!(uuids, [created.assertions[1], created.assertions[0]]);
    let excluded = &active.assertions[0];
    assert_eq!(excluded.claim, Claim::Excluded);
    assert_eq!(excluded.rationale.as_deref(), Some("A sample"));
    assert_eq!(excluded.files[0].data_source.name, "media");
    assert_eq!(excluded.files[0].path, "b.mkv");

    // Retracting one in another changeset takes it out of effect.
    let undo = client.create_changeset(&agent()).await.unwrap();
    client
        .retract(
            undo.uuid,
            &NewRetraction {
                assertion: created.assertions[1],
            },
        )
        .await
        .unwrap();
    client.accept_changeset(undo.uuid).await.unwrap();
    let active = client.list_assertions().await.unwrap();
    let uuids: Vec<_> = active.assertions.iter().map(|a| a.uuid).collect();
    assert_eq!(uuids, [created.assertions[0]]);
}

/// An export from another instance names its data sources, whose UUIDs this
/// one doesn't have, so its files are matched by name. What `wst assertions
/// import` and the Assertions page's Import do.
#[tokio::test]
async fn an_export_imports_as_an_open_changeset() {
    let (ctx, source, _tmp) = library(&["a.mkv", "b.mkv"]).await;
    let client = client(&ctx);
    let elsewhere = "11111111-2222-3333-4444-555555555555".parse().unwrap();
    let exported_file = |data_source: DataSource, path: &str| AssertedFile {
        data_source,
        path: path.into(),
        retracted_at: None,
    };
    // Listed newest first, as an export is; made oldest first.
    let exported = |tmdb_file: AssertedFile| ImportAssertions {
        author: "import".into(),
        description: "Import from assertions.json".into(),
        assertions: vec![
            Assertion {
                uuid: elsewhere,
                claim: Claim::Excluded,
                files: vec![exported_file(
                    DataSource {
                        uuid: elsewhere,
                        name: "media".into(),
                    },
                    "b.mkv",
                )],
                created_at: 200,
                rationale: None,
                confidence: None,
            },
            Assertion {
                uuid: elsewhere,
                claim: Claim::MovieExternalId(ExternalId::Tmdb(949)),
                files: vec![tmdb_file],
                created_at: 100,
                rationale: Some("Runtime matches".into()),
                confidence: Some(0.9),
            },
        ],
    };

    // A data source this instance has by neither UUID nor name.
    let error = client
        .import_assertions(&exported(exported_file(
            DataSource {
                uuid: elsewhere,
                name: "films".into(),
            },
            "a.mkv",
        )))
        .await
        .unwrap_err();
    let (status, code, message) = refused(error);
    assert_eq!((status, code), (409, ErrorCode::Conflict));
    assert!(message.contains("\"films\""), "{message}");
    let changesets = client
        .list_changesets(&ChangesetsQuery { state: None })
        .await
        .unwrap();
    assert!(changesets.changesets.is_empty());

    // By UUID when it's ours, whatever it was called there.
    let imported = client
        .import_assertions(&exported(exported_file(
            DataSource {
                uuid: source.0,
                name: "films".into(),
            },
            "a.mkv",
        )))
        .await
        .unwrap();
    let changeset = client.get_changeset(imported.changeset).await.unwrap();
    assert_eq!(changeset.state, ChangesetState::Open);
    assert_eq!(changeset.author, "import");
    assert_eq!(changeset.description, "Import from assertions.json");
    // The changeset lists them newest first too.
    let uuids: Vec<_> = changeset.assertions.iter().map(|a| a.uuid).collect();
    assert_eq!(uuids, [imported.assertions[1], imported.assertions[0]]);
    let tmdb = &changeset.assertions[1];
    assert_eq!(tmdb.claim, Claim::MovieExternalId(ExternalId::Tmdb(949)));
    assert_eq!(tmdb.rationale.as_deref(), Some("Runtime matches"));
    assert_eq!(tmdb.confidence, Some(0.9));
    assert_eq!(tmdb.files[0].data_source.uuid, source.0);
    assert_eq!(tmdb.files[0].data_source.name, "media");
    assert_eq!(tmdb.files[0].path, "a.mkv");
    assert_eq!(changeset.assertions[0].files[0].path, "b.mkv");

    client.accept_changeset(imported.changeset).await.unwrap();
    let active = client.list_assertions().await.unwrap();
    assert_eq!(active.assertions.len(), 2);

    let error = client
        .import_assertions(&ImportAssertions {
            author: "import".into(),
            description: String::new(),
            assertions: vec![],
        })
        .await
        .unwrap_err();
    let (status, code, _) = refused(error);
    assert_eq!((status, code), (400, ErrorCode::BadRequest));
}

/// Bodies the client's methods wouldn't send, so they go as JSON.
#[tokio::test]
async fn changeset_errors_are_json() {
    let (ctx, _, _tmp) = library(&["a.mkv"]).await;
    let client = client(&ctx);
    let changeset = client.create_changeset(&agent()).await.unwrap();
    let path = format!("/changesets/{}", changeset.uuid);
    let nil = "00000000-0000-0000-0000-000000000000";

    let cases = [
        (
            "/changesets".to_string(),
            json!({"author": ""}),
            400,
            ErrorCode::BadRequest,
        ),
        (
            "/changesets".into(),
            json!({"who": "agent"}),
            400,
            ErrorCode::BadRequest,
        ),
        (
            format!("{path}/assertions"),
            json!({"assertions": [{"files": [], "claim": {"kind": "special"}}]}),
            400,
            ErrorCode::BadRequest,
        ),
        (
            format!("{path}/assertions"),
            json!({"assertions": [{"files": [], "claim": {"kind": "made_up"}}]}),
            400,
            ErrorCode::BadRequest,
        ),
        (
            format!("{path}/retractions"),
            json!({"assertion": nil}),
            404,
            ErrorCode::NotFound,
        ),
        (
            format!("/changesets/{nil}/accept"),
            json!({}),
            404,
            ErrorCode::NotFound,
        ),
    ];
    for (path, body, expected_status, expected_code) in cases {
        let error = client
            .post::<serde_json::Value>(&path, &body)
            .await
            .unwrap_err();
        let (status, code, _) = refused(error);
        assert_eq!((status, code), (expected_status, expected_code), "{path}");
    }
}

#[tokio::test]
async fn a_changesets_diff_shows_what_accepting_it_would_change() {
    let (ctx, source, _tmp) = library(&["a.mkv", "b.mkv"]).await;
    let client = client(&ctx);
    let changeset = client.create_changeset(&agent()).await.unwrap();
    client
        .add_assertions(
            changeset.uuid,
            &NewAssertions {
                assertions: vec![assertion(
                    source,
                    "a.mkv",
                    Claim::MovieExternalId(ExternalId::Tmdb(949)),
                )],
            },
        )
        .await
        .unwrap();

    let diff = client.diff_changeset(changeset.uuid).await.unwrap();
    assert!(diff.base.canonical);
    assert!(!diff.other.canonical);
    let [change] = &diff.changes[..] else {
        panic!("one change: {diff:?}");
    };
    assert_eq!(change.kind, WorkChangeKind::Added);
    assert_eq!(change.after[0].external_ids, [ExternalId::Tmdb(949)]);
    let paths: Vec<_> = change.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["a.mkv"]);

    // Asked again, it's the same interpretation, which a diff of
    // interpretations shows the same.
    let again = client.diff_changeset(changeset.uuid).await.unwrap();
    assert_eq!(again.other.uuid, diff.other.uuid);
    let direct = client
        .diff(&DiffQuery {
            base: None,
            other: diff.other.uuid,
        })
        .await
        .unwrap();
    assert_eq!(direct.changes.len(), 1);
}

#[tokio::test]
async fn done_marks_are_listed_only_on_request() {
    let (ctx, source, _tmp) = library(&["a.mkv", "b.mkv"]).await;
    let client = client(&ctx);
    mark_files(
        &ctx,
        NewMark {
            label: "file a.mkv".into(),
            note: "a sample".into(),
            files: vec![PathId::new(source, "a.mkv")],
        },
    )
    .await
    .unwrap();
    let marks = client
        .list_marks(&MarksQuery { done: false })
        .await
        .unwrap();
    let [mark] = &marks.marks[..] else {
        panic!("one mark: {marks:?}")
    };
    assert_eq!(mark.note, "a sample");
    assert_eq!(mark.progress, MarkProgress::Open);
    assert_eq!(
        (mark.files[0].data_source.uuid, mark.files[0].path.as_str()),
        (source.0, "a.mkv")
    );

    let changeset = client.create_changeset(&agent()).await.unwrap();
    client
        .add_assertions(
            changeset.uuid,
            &NewAssertions {
                assertions: vec![assertion(source, "a.mkv", Claim::Excluded)],
            },
        )
        .await
        .unwrap();
    client.accept_changeset(changeset.uuid).await.unwrap();

    let marks = client
        .list_marks(&MarksQuery { done: false })
        .await
        .unwrap();
    assert!(marks.marks.is_empty(), "{marks:?}");
    let marks = client.list_marks(&MarksQuery { done: true }).await.unwrap();
    assert_eq!(marks.marks[0].progress, MarkProgress::Done);
}

/// Every `$ref` in `value`, e.g. `#/components/schemas/Claim`.
fn refs<'a>(value: &'a serde_json::Value, found: &mut Vec<&'a str>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(target)) = map.get("$ref") {
                found.push(target);
            }
            map.values().for_each(|child| refs(child, found));
        }
        serde_json::Value::Array(items) => items.iter().for_each(|item| refs(item, found)),
        _ => {}
    }
}

/// The OpenAPI document names the routes as they're served, and every schema
/// it refers to is in it: utoipa only collects those that a path's bodies
/// reach, so one referred to by name alone can go missing.
#[tokio::test]
async fn the_openapi_document_is_whole() {
    let core = CoreContext::new_in_memory().await.unwrap();
    let ctx = AppContext::new(core, Default::default(), "/fake/data");

    let spec = client(&ctx)
        .get::<serde_json::Value>("/openapi.json", &())
        .await
        .unwrap();

    let paths = spec["paths"].as_object().unwrap();
    assert!(
        paths.contains_key("/api/v1/changesets/{uuid}/accept"),
        "{:?}",
        paths.keys()
    );
    assert!(paths["/api/v1/changesets"]["post"].is_object());

    let mut found = Vec::new();
    refs(&spec, &mut found);
    assert!(found.contains(&"#/components/schemas/ErrorResponse"));
    for target in found {
        let name = target.strip_prefix("#/components/schemas/").unwrap();
        assert!(
            spec["components"]["schemas"][name].is_object(),
            "{target} isn't in the document"
        );
    }
}

#[tokio::test]
async fn health_answers_before_anything_is_ingested() {
    let core = CoreContext::new_in_memory().await.unwrap();
    let ctx = AppContext::new(core, Default::default(), "/fake/data");

    let health = client(&ctx).health().await.unwrap();
    assert_eq!(health.status, HealthStatus::Ok);
    assert_eq!(health.version, env!("CARGO_PKG_VERSION"));
}

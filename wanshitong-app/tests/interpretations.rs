mod common;

use common::wait_for_interpretations;
use std::{path::PathBuf, time::Duration};
use tempfile::tempdir;
use wanshitong_app::commands::{
    IngestStart, accept_changeset, assert_claim, correct_in_draft, create_data_source,
    create_interpretation, find_draft, make_interpretation_canonical, retract_assertion,
    start_ingest,
};
use wanshitong_app::domain::PathId;
use wanshitong_app::domain::{
    ClaimPredicate, DataSourceConfig, DiscKind, ExternalId, FileSystemSource, InterpretationUuid,
};
use wanshitong_app::interpretations::{InterpretationState, InterpretationStatus};
use wanshitong_app::queries::{
    FileSearchQuery, FileTreeList, FileTreeQuery, MovieSearchQuery, get_file, get_interpretation,
    list_active_assertions, list_file_tree, list_files, list_interpretations, list_movies,
    wait_for_ingest,
};
use wanshitong_app::{AppContext, CoreContext, CoreErrorContext};

/// Polls until the interpretation stops running.
async fn wait_for_interpretation(
    ctx: &AppContext,
    uuid: InterpretationUuid,
) -> InterpretationStatus {
    for _ in 0..200 {
        let details = get_interpretation(ctx, uuid).await.unwrap().unwrap();
        if details.state != InterpretationState::Running {
            return details;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("interpretation {uuid} still running");
}

#[tokio::test]
async fn interprets_in_the_background_and_becomes_canonical_on_request() {
    let tmp = tempdir().unwrap();
    tokio::fs::write(tmp.path().join("notes.txt"), "not a movie")
        .await
        .unwrap();
    let core = CoreContext::new_in_memory().await.unwrap();
    let ctx = AppContext::new(core, Default::default(), "/fake/data");
    let config = DataSourceConfig::FileSystem(FileSystemSource {
        root_path: tmp.path().into(),
        follow_symlinks: false,
    });
    let source = create_data_source(&ctx, "media", config).await.unwrap();

    // An ingest interprets its results and makes them canonical.
    let IngestStart::Started(ingest) = start_ingest(&ctx, source.uuid).await.unwrap() else {
        panic!("no ingest of it was running");
    };
    wait_for_ingest(&ctx, ingest).await.unwrap();
    let after_ingest = list_interpretations(&ctx).await.unwrap();
    assert_eq!(after_ingest.len(), 1);
    // Deriving the catalog isn't part of the ingest.
    let after_ingest = wait_for_interpretation(&ctx, after_ingest[0].interpretation.uuid).await;
    assert_eq!(after_ingest.interpretation.name, "After ingest of media");
    assert_eq!(after_ingest.state, InterpretationState::Succeeded);
    assert!(after_ingest.interpretation.is_canonical);

    // A new interpretation runs without taking over the catalog...
    let uuid = create_interpretation(&ctx, "Trying something", false)
        .await
        .unwrap();
    let details = wait_for_interpretation(&ctx, uuid).await;
    assert_eq!(details.state, InterpretationState::Succeeded);
    assert!(!details.interpretation.is_canonical);
    assert_eq!(details.interpretation.ingests.len(), 1);
    assert_eq!(details.interpretation.ingests[0].ingest_uuid, ingest);

    // ...until asked to.
    make_interpretation_canonical(&ctx, uuid).await.unwrap();
    let canonical: Vec<_> = list_interpretations(&ctx)
        .await
        .unwrap()
        .into_iter()
        .filter(|details| details.interpretation.is_canonical)
        .map(|details| details.interpretation.uuid)
        .collect();
    assert_eq!(canonical, vec![uuid]);

    // Names are validated before anything runs.
    let err = create_interpretation(&ctx, " ", true).await.unwrap_err();
    assert!(matches!(
        err.core_context(),
        Some(CoreErrorContext::InvalidInterpretationName(_))
    ));
    assert_eq!(list_interpretations(&ctx).await.unwrap().len(), 2);
}

#[tokio::test]
async fn assertions_take_effect_in_a_new_canonical_interpretation() {
    let tmp = tempdir().unwrap();
    tokio::fs::write(
        tmp.path().join("Redline ITBD Remux.mkv"),
        "not really a movie",
    )
    .await
    .unwrap();
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
    let movies = |ctx| async move {
        list_movies(ctx, &MovieSearchQuery::default())
            .await
            .unwrap()
            .movies
            .into_iter()
            .map(|movie| (movie.title, movie.tmdb_id))
            .collect::<Vec<_>>()
    };
    // Without video streams, heuristics find no movie here.
    assert_eq!(movies(&ctx).await, vec![]);

    let rel_path = PathBuf::from("Redline ITBD Remux.mkv");
    let claims = [
        ClaimPredicate::MovieTitle("Redline".into()),
        ClaimPredicate::MovieExternalId(ExternalId::Tmdb(71883)),
    ];
    let mut interpretation = None;
    for claim in claims {
        interpretation = Some(
            assert_claim(&ctx, &[PathId::new(source.uuid, rel_path.clone())], claim)
                .await
                .unwrap(),
        );
    }
    let details = wait_for_interpretation(&ctx, interpretation.unwrap()).await;
    assert_eq!(details.state, InterpretationState::Succeeded);
    assert_eq!(
        details.interpretation.name,
        "Asserted TMDB 71883 for \"Redline ITBD Remux.mkv\""
    );
    assert_eq!(movies(&ctx).await, vec![("Redline".into(), Some(71883))]);

    // Removing one takes effect the same way.
    let title = list_active_assertions(&ctx)
        .await
        .unwrap()
        .into_iter()
        .find(|a| matches!(a.claim, ClaimPredicate::MovieTitle(_)))
        .unwrap();
    let interpretation = retract_assertion(&ctx, title.uuid).await.unwrap();
    wait_for_interpretation(&ctx, interpretation).await;
    assert_eq!(movies(&ctx).await, vec![]);
}

#[tokio::test]
async fn names_interpretations_after_files_with_long_multibyte_names() {
    let file_name = "(アニメ)(映画)東映まんがまつり グリム童話 金の鳥(1987) (東映チャンネルHD 1440x1080 x265).mp4";
    let tmp = tempdir().unwrap();
    tokio::fs::write(tmp.path().join(file_name), "")
        .await
        .unwrap();
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

    let claim = ClaimPredicate::MovieExternalId(ExternalId::Tmdb(466000));
    let interpretation = assert_claim(&ctx, &[PathId::new(source.uuid, file_name)], claim)
        .await
        .unwrap();
    let details = wait_for_interpretation(&ctx, interpretation).await;
    assert_eq!(details.state, InterpretationState::Succeeded);
    assert!(
        details
            .interpretation
            .name
            .starts_with("Asserted TMDB 466000 for \"(アニメ)")
    );
}

/// Writes a second of video, which nothing will identify without a knowledge
/// base.
fn write_video(path: &std::path::Path) {
    let status = std::process::Command::new("ffmpeg")
        .args(["-loglevel", "error", "-f", "lavfi", "-i"])
        .arg("testsrc=duration=1:size=64x48:rate=5")
        .arg(path)
        .status()
        .expect("ffmpeg runs");
    assert!(status.success(), "ffmpeg made {}", path.display());
}

#[tokio::test]
async fn only_identified_works_are_in_the_catalog() {
    let tmp = tempdir().unwrap();
    let name = "Obscure Film (1999).mkv";
    write_video(&tmp.path().join(name));
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
    let catalog = async || {
        let file = get_file(&ctx, source.uuid, name.as_ref())
            .await
            .unwrap()
            .unwrap();
        let movies: Vec<_> = file.movies.into_iter().map(|m| m.title).collect();
        let unidentified: Vec<_> = list_files(&ctx, &FileSearchQuery::default())
            .await
            .unwrap()
            .files
            .into_iter()
            .map(|f| f.rel_path)
            .collect();
        (movies, unidentified, file.audit_log.is_some())
    };

    // Its name suggests a movie, but without a knowledge base nothing says
    // which, so it's an unidentified video. What was guessed is kept.
    assert_eq!(catalog().await, (vec![], vec![name.to_string()], true));

    // Identifying it makes it a movie.
    let claim = ClaimPredicate::MovieExternalId(ExternalId::Tmdb(1));
    let interpretation = assert_claim(&ctx, &[PathId::new(source.uuid, name)], claim)
        .await
        .unwrap();
    wait_for_interpretation(&ctx, interpretation).await;
    assert_eq!(
        catalog().await,
        (vec!["obscure film".to_string()], vec![], true)
    );
}

#[tokio::test]
async fn an_excluded_disc_leaves_the_catalog() {
    let tmp = tempdir().unwrap();
    let (index, stream) = ("D2/BDMV/index.bdmv", "D2/BDMV/STREAM/00001.m2ts");
    std::fs::create_dir_all(tmp.path().join("D2/BDMV/STREAM")).unwrap();
    std::fs::write(tmp.path().join(index), "INDX0200").unwrap();
    write_video(&tmp.path().join(stream));
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
    let index_files = || vec![PathId::new(source.uuid, index)];
    let correct = async |claims| {
        correct_in_draft(&ctx, &index_files(), claims)
            .await
            .unwrap();
        let draft = find_draft(&ctx).await.unwrap().unwrap();
        let interpretation = accept_changeset(&ctx, draft).await.unwrap();
        wait_for_interpretation(&ctx, interpretation).await;
    };
    let catalog = async || {
        let movies = async |path: &str| -> Vec<String> {
            let file = get_file(&ctx, source.uuid, path.as_ref())
                .await
                .unwrap()
                .unwrap();
            file.movies.into_iter().map(|m| m.title).collect()
        };
        let unidentified: Vec<_> = list_files(&ctx, &FileSearchQuery::default())
            .await
            .unwrap()
            .files
            .into_iter()
            .map(|f| f.rel_path)
            .collect();
        (movies(index).await, movies(stream).await, unidentified)
    };

    // Taken for a movie, e.g. by a knowledge base finding its directory's name,
    // the disc is its main file. Its stream is no file of its own.
    correct(vec![ClaimPredicate::MovieExternalId(ExternalId::Tmdb(1))]).await;
    let d2 = vec!["d2".to_string()];
    assert_eq!(catalog().await, (d2, vec![], vec![]));

    // Excluded, the disc leaves the catalog, and isn't left unidentified.
    correct(vec![ClaimPredicate::Excluded]).await;
    assert_eq!(catalog().await, (vec![], vec![], vec![]));

    // Identified again, it's back.
    correct(vec![ClaimPredicate::MovieExternalId(ExternalId::Tmdb(1))]).await;
    assert_eq!(catalog().await.0, ["d2"]);
}

#[tokio::test]
async fn the_files_list_unidentified_videos_unless_asked_for_more() {
    let tmp = tempdir().unwrap();
    write_video(&tmp.path().join("unknown.mkv"));
    write_video(&tmp.path().join("heat.mkv"));
    std::fs::write(tmp.path().join("notes.txt"), "not a video").unwrap();
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
    let claim = ClaimPredicate::MovieExternalId(ExternalId::Tmdb(949));
    let interpretation = assert_claim(&ctx, &[PathId::new(source.uuid, "heat.mkv")], claim)
        .await
        .unwrap();
    wait_for_interpretation(&ctx, interpretation).await;
    let files = async |identified, non_videos| -> Vec<String> {
        let query = FileSearchQuery {
            identified,
            non_videos,
            ..Default::default()
        };
        let files = list_files(&ctx, &query).await.unwrap().files;
        files.into_iter().map(|f| f.rel_path).collect()
    };

    assert_eq!(files(false, false).await, ["unknown.mkv"]);
    assert_eq!(files(true, false).await, ["heat.mkv", "unknown.mkv"]);
    assert_eq!(files(false, true).await, ["notes.txt", "unknown.mkv"]);
    assert_eq!(
        files(true, true).await,
        ["heat.mkv", "notes.txt", "unknown.mkv"]
    );
}

#[tokio::test]
async fn the_file_tree_lists_a_folders_children_with_counts_page_by_page() {
    let tmp = tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("Show/S01")).unwrap();
    std::fs::create_dir_all(tmp.path().join("Other")).unwrap();
    for video in [
        "Show/S01/a.mkv",
        "Show/S01/b.mkv",
        "Show/c.mkv",
        "Other/x.mkv",
        "top.mkv",
    ] {
        write_video(&tmp.path().join(video));
    }
    std::fs::write(tmp.path().join("Show/S01/a.srt"), "1\n").unwrap();
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
    let children = async |folder: Option<&str>, non_videos, after, limit| {
        let query = FileTreeQuery {
            folder: folder.map(|path| (source.uuid, path.to_string())),
            work: None,
            identified: false,
            non_videos,
            after,
            limit,
        };
        list_file_tree(&ctx, &query).await.unwrap()
    };
    let names = |list: &FileTreeList| -> Vec<(String, bool, i64)> {
        list.entries
            .iter()
            .map(|e| (e.name.clone(), e.is_folder, e.file_count))
            .collect()
    };
    let entry = |name: &str, is_folder, count| (name.to_string(), is_folder, count);

    // By name, ignoring case, each folder with how many unidentified videos it holds.
    let top = children(None, false, None, 50).await;
    assert_eq!(
        names(&top),
        [
            entry("Other", true, 1),
            entry("Show", true, 3),
            entry("top.mkv", false, 1)
        ]
    );
    let show = children(Some("Show"), false, None, 50).await;
    assert_eq!(
        names(&show),
        [entry("c.mkv", false, 1), entry("S01", true, 2)]
    );
    assert_eq!(show.entries[0].path, "Show/c.mkv");
    // Non-videos count too, when asked for.
    let show = children(Some("Show"), true, None, 50).await;
    assert_eq!(names(&show)[1], entry("S01", true, 3));

    // A page at a time.
    let first = children(None, false, None, 2).await;
    assert_eq!(names(&first).len(), 2);
    let rest = children(None, false, first.next.clone(), 2).await;
    assert_eq!(names(&rest), [entry("top.mkv", false, 1)]);
    assert_eq!(rest.next, None);

    // Marking a folder marks the files under it.
    let query = FileSearchQuery {
        folder: Some((Some(source.uuid), "Show".into())),
        ..Default::default()
    };
    let mut under: Vec<_> = list_files(&ctx, &query)
        .await
        .unwrap()
        .files
        .into_iter()
        .map(|f| f.rel_path)
        .collect();
    under.sort();
    assert_eq!(under, ["Show/S01/a.mkv", "Show/S01/b.mkv", "Show/c.mkv"]);
}

#[tokio::test]
async fn a_disc_is_one_file_from_ingest_on() {
    let tmp = tempdir().unwrap();
    let (index, stream) = (
        "Some Film (1999)/BDMV/index.bdmv",
        "Some Film (1999)/BDMV/STREAM/00001.m2ts",
    );
    std::fs::create_dir_all(tmp.path().join("Some Film (1999)/BDMV/STREAM")).unwrap();
    std::fs::create_dir_all(tmp.path().join("Some Film (1999)/BDMV/PLAYLIST")).unwrap();
    std::fs::write(tmp.path().join(index), "INDX0200").unwrap();
    std::fs::write(
        tmp.path().join("Some Film (1999)/BDMV/PLAYLIST/00000.mpls"),
        "MPLS0200",
    )
    .unwrap();
    write_video(&tmp.path().join(stream));
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
    let every_file = FileSearchQuery {
        identified: true,
        non_videos: true,
        ..Default::default()
    };
    let paths = async |query: &FileSearchQuery| -> Vec<String> {
        let files = list_files(&ctx, query).await.unwrap().files;
        files.into_iter().map(|f| f.rel_path).collect()
    };

    // The disc's index file stands for it: its other files are nowhere.
    assert_eq!(paths(&every_file).await, [index]);
    // Unidentified, the disc is one unidentified video.
    assert_eq!(paths(&FileSearchQuery::default()).await, [index]);
    let listed = list_files(&ctx, &FileSearchQuery::default()).await.unwrap();
    let disc = listed.files[0].disc.unwrap();
    assert_eq!(disc.kind, DiscKind::BluRay);
    assert_eq!(disc.file_count, 3);
    assert!(disc.duration.is_some());

    // In the tree, it's one entry, where its files are, standing for them.
    let query = |folder: &str| FileTreeQuery {
        folder: Some((source.uuid, folder.to_string())),
        work: None,
        identified: false,
        non_videos: false,
        after: None,
        limit: 50,
    };
    let entries = list_file_tree(&ctx, &query("Some Film (1999)"))
        .await
        .unwrap()
        .entries;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "BDMV");
    assert_eq!(entries[0].path, index);
    assert!(!entries[0].is_folder && entries[0].disc.is_some());

    // Identified, it's its movie's main file, and its only file.
    let claim = ClaimPredicate::MovieExternalId(ExternalId::Tmdb(1));
    let interpretation = assert_claim(&ctx, &[PathId::new(source.uuid, index)], claim)
        .await
        .unwrap();
    wait_for_interpretation(&ctx, interpretation).await;
    let file = get_file(&ctx, source.uuid, index.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(file.movies.len(), 1);
    assert_eq!(file.movies[0].title, "some film");
    assert!(paths(&FileSearchQuery::default()).await.is_empty());
}

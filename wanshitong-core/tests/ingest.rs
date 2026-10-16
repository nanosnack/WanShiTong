mod common;

use common::ingest_files;
use std::collections::BTreeSet;
use wanshitong_core::CoreContext;
use wanshitong_core::commands::create_interpretation;
use wanshitong_core::queries::list_facts;

/// The sink writes files in batches, so an ingest of more files than fit in
/// one must still record each of them exactly once.
#[tokio::test]
async fn ingest_records_every_file_across_batches() {
    let ctx = CoreContext::new_in_memory().await.unwrap();
    let paths: Vec<String> = (0..2500).map(|i| format!("file{i}.mkv")).collect();
    let rel_paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    let (source, ingest) = ingest_files(&ctx, "Movies", &rel_paths).await;

    let interpretation = create_interpretation(&ctx, "test", &[(source, ingest)])
        .await
        .unwrap()
        .row_id;
    let files = list_facts(&ctx, interpretation).await.unwrap().files;
    let recorded: BTreeSet<_> = files
        .iter()
        .map(|(path, _)| path.rel_path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(files.len(), paths.len());
    assert_eq!(recorded, paths.into_iter().collect());
}

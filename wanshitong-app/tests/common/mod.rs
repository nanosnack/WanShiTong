//! Fixtures shared by the integration tests.
//!
//! Each test binary compiles this module and uses only some of it.
#![allow(dead_code)]

use std::time::Duration;
use wanshitong_app::AppContext;
use wanshitong_app::interpretations::InterpretationState;
use wanshitong_app::queries::list_interpretations;

/// Polls until no interpretation is running, e.g. the one an ingest starts
/// when it finishes.
pub async fn wait_for_interpretations(ctx: &AppContext) {
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

/// Polls until `n` ingests have finished and none is running, then returns
/// them, oldest first.
pub async fn wait_for_finished_ingests(
    ctx: &AppContext,
    n: usize,
) -> Vec<wanshitong_app::ingest::IngestStatus> {
    use wanshitong_app::ingest::IngestState;
    use wanshitong_app::queries::list_ingests;
    for _ in 0..200 {
        let ingests = list_ingests(ctx).await.unwrap();
        let running = ingests
            .iter()
            .any(|i| matches!(i.state(), IngestState::Pending));
        if ingests.len() >= n && !running {
            assert_eq!(ingests.len(), n, "more ingests than expected");
            // The listing is newest first, and times are whole seconds, so
            // ingests of the same second come in table order.
            let mut ingests = ingests;
            ingests.sort_by_key(|i| (i.ingest.started_at, i.ingest.id.0));
            return ingests;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("{n} ingests didn't finish");
}

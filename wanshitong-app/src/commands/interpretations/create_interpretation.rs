use crate::commands::{change_canonical_interpretation, prune_interpretations};
use crate::context::TaskId;
use crate::interpret::generate_catalog;
use crate::interpret::knowledge_base::KnowledgeBase;
use crate::interpretations::RunningInterpretation;
use crate::knowledge_base_types::Tmdb;
use crate::report_error;
use crate::{AppContext, AppError};
use futures::FutureExt;
use tracing::{Instrument, info_span};
use uuid::Uuid;
use wanshitong_core::commands::{
    NewInterpretation, create_interpretation as create_interpretation_core, finish_interpretation,
    record_interpretation_errors,
};
use wanshitong_core::domain::InterpretationUuid;
use wanshitong_core::queries::list_latest_successful_ingests;

/// Starts interpreting the latest successful ingest of every data source, in
/// the background. If `make_canonical`, the interpretation becomes canonical
/// once it succeeds.
pub async fn create_interpretation(
    ctx: &AppContext,
    name: &str,
    make_canonical: bool,
) -> Result<InterpretationUuid, AppError> {
    let new_interpretation = create_latest_interpretation(ctx, name).await?;
    let uuid = new_interpretation.uuid;
    spawn_interpretation(ctx, new_interpretation, make_canonical).await;
    Ok(uuid)
}

/// Derives a created interpretation's catalog in the background. If
/// `make_canonical`, the interpretation becomes canonical once it succeeds.
pub(crate) async fn spawn_interpretation(
    ctx: &AppContext,
    new_interpretation: NewInterpretation,
    make_canonical: bool,
) {
    let running = ctx.running_interpretations.start(new_interpretation.row_id);
    let task_id = TaskId(Uuid::new_v4());
    // Lock task container before spawning to ensure we insert the task before
    // the spawned task tries to remove itself.
    let mut tasks = ctx.tasks.lock().await;
    let task = tokio::spawn(panic_safe_interpretation_task(
        ctx.clone(),
        task_id,
        running,
        make_canonical,
    ));
    tasks.insert(task_id, task);
}

/// An interpretation of the latest successful ingest of every data source.
pub(crate) async fn create_latest_interpretation(
    ctx: &AppContext,
    name: &str,
) -> Result<NewInterpretation, AppError> {
    let sources = list_latest_successful_ingests(&ctx.core).await?;
    Ok(create_interpretation_core(&ctx.core, name, &sources).await?)
}

async fn panic_safe_interpretation_task(
    ctx: AppContext,
    task_id: TaskId,
    running: RunningInterpretation,
    make_canonical: bool,
) {
    let id = running.id();
    let result = std::panic::AssertUnwindSafe(interpret(&ctx, running, make_canonical))
        .catch_unwind()
        .await;
    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => report_error!(e, %id, "Failed to finish interpretation"),
        Err(_) => {
            // Unwind error is not useful to users; give generic error message.
            let error = Some("interpretation task panicked");
            if let Err(e) = finish_interpretation(&ctx.core, id, error).await {
                report_error!(e, %id, "Failed to finish interpretation");
            }
        }
    }
    ctx.tasks.lock().await.remove(&task_id);
}

/// Derives an interpretation's catalog and records how that went. If it
/// succeeded and `make_canonical`, the interpretation becomes canonical.
///
/// A failure to derive the catalog is recorded on the interpretation rather
/// than returned, since it's something to show about the interpretation.
async fn interpret(
    ctx: &AppContext,
    running: RunningInterpretation,
    make_canonical: bool,
) -> Result<(), AppError> {
    // One at a time, so that interpretations become canonical in the order
    // they were started.
    let _guard = ctx.interpreting.lock().await;
    let id = running.id();

    let tmdb = ctx.tmdb_client().map(Tmdb::new);
    // So that what's logged while deriving the catalog says which
    // interpretation it's about.
    let mut errors = Vec::new();
    let result = generate_catalog(
        &ctx.core,
        tmdb.as_ref().map(|t| t as &dyn KnowledgeBase),
        id,
        &mut errors,
    )
    .instrument(info_span!("interpretation", %id))
    .await;
    // Recorded even if deriving the catalog failed, as they may say why.
    if let Err(e) = record_interpretation_errors(&ctx.core, id, &errors).await {
        report_error!(e, %id, "Failed to record the interpretation's errors");
    }

    let error = result.err().map(|e| {
        report_error!(e, %id, "Interpretation failed");
        format!("{e:#}")
    });
    finish_interpretation(&ctx.core, id, error.as_deref()).await?;
    // Still running until then, so it's never seen succeeded but not yet
    // canonical.
    if error.is_none() && make_canonical {
        change_canonical_interpretation(ctx, id).await?;
    }
    // Now that this one is finished and, if it was to be, canonical, older
    // ones can go.
    if let Err(e) = prune_interpretations(ctx).await {
        report_error!(e, "Failed to prune old interpretations");
    }
    drop(running);
    Ok(())
}

use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{InterpretationRowId, ViewSyncStats, ViewUuid},
};
use jiff::Timestamp;

pub struct NewViewMaterialization {
    pub view: ViewUuid,
    pub interpretation_id: Option<InterpretationRowId>,
    pub started_at: Timestamp,
    pub stopped_at: Timestamp,
    /// The error message on failure.
    pub result: Result<ViewSyncStats, String>,
}

/// Records an attempt to materialize a view. The latest one is the view's
/// current state.
pub async fn record_view_materialization(
    ctx: &CoreContext,
    materialization: NewViewMaterialization,
) -> Result<(), CoreError> {
    let NewViewMaterialization {
        view,
        interpretation_id,
        started_at,
        stopped_at,
        result,
    } = materialization;

    let started_at = started_at.as_second();
    let stopped_at = stopped_at.as_second();
    let success = result.is_ok();
    let (stats, error) = match result {
        Ok(stats) => (Some(stats), None),
        Err(error) => (None, Some(error)),
    };
    let links_created = stats.map(|s| s.links_created as i64);
    let links_updated = stats.map(|s| s.links_updated as i64);
    let links_removed = stats.map(|s| s.links_removed as i64);

    let inserted = sqlx::query!(
        r#"
        INSERT INTO view_materializations (
            view_id,
            interpretation_id,
            started_at,
            stopped_at,
            success,
            links_created,
            links_updated,
            links_removed,
            error
        )
        SELECT id, ?, ?, ?, ?, ?, ?, ?, ?
        FROM views
        WHERE uuid = ?
        "#,
        interpretation_id,
        started_at,
        stopped_at,
        success,
        links_created,
        links_updated,
        links_removed,
        error,
        view,
    )
    .execute(&ctx.writer)
    .await
    .map_err(CoreErrorContext::from)?;

    if inserted.rows_affected() == 0 {
        error_stack::bail!(CoreErrorContext::ViewNotFound(view));
    }
    Ok(())
}

//! Views: materializing the canonical catalog for other programs to read.

mod jellyfin;
mod layout;
mod plex;
mod sync;

pub use sync::SyncError;

use crate::{AppContext, AppError};
use jellyfin::Jellyfin;
use layout::{Convention, plan_movies, plan_shows};
use plex::Plex;
use sync::{remove_symlink_tree, sync_symlink_tree};
use wanshitong_core::{
    domain::{InterpretationRowId, View, ViewConfig, ViewSyncStats},
    queries::{list_movies_for_view, list_tv_series_for_view},
};

/// A view along with how it compares to the canonical catalog.
pub struct ViewStatus {
    pub view: View,
    pub state: ViewState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewState {
    NeverMaterialized,
    /// The latest attempt failed, so the view may be partly updated.
    Failed,
    /// Materialized from an interpretation that is no longer canonical.
    Stale,
    UpToDate,
}

impl ViewStatus {
    pub(crate) fn new(view: View, canonical: Option<InterpretationRowId>) -> Self {
        let state = match &view.latest_materialization {
            None => ViewState::NeverMaterialized,
            Some(latest) if latest.result.is_err() => ViewState::Failed,
            Some(latest) if latest.interpretation_id != canonical => ViewState::Stale,
            Some(_) => ViewState::UpToDate,
        };
        Self { view, state }
    }
}

/// Brings a view's files in line with an interpretation's catalog.
pub(crate) async fn materialize(
    ctx: &AppContext,
    config: &ViewConfig,
    interpretation_id: InterpretationRowId,
) -> Result<ViewSyncStats, AppError> {
    match config {
        ViewConfig::Jellyfin(_) => {
            materialize_with::<Jellyfin>(ctx, config, interpretation_id).await
        }
        ViewConfig::Plex(_) => materialize_with::<Plex>(ctx, config, interpretation_id).await,
    }
}

/// Links a movies library and a TV shows library, named by `C`.
async fn materialize_with<C: Convention>(
    ctx: &AppContext,
    config: &ViewConfig,
    interpretation_id: InterpretationRowId,
) -> Result<ViewSyncStats, AppError> {
    let movies = list_movies_for_view(&ctx.core, interpretation_id).await?;
    let series = list_tv_series_for_view(&ctx.core, interpretation_id).await?;
    let libraries = [
        (config.movies_path(), plan_movies::<C>(&movies)),
        (config.shows_path(), plan_shows::<C>(&series)),
    ];
    let stats = tokio::task::spawn_blocking(move || {
        let mut total = ViewSyncStats::default();
        for (path, desired) in &libraries {
            let stats = sync_symlink_tree(path, desired)?;
            total.links_created += stats.links_created;
            total.links_updated += stats.links_updated;
            total.links_removed += stats.links_removed;
        }
        Ok::<_, SyncError>(total)
    })
    .await
    .map_err(|e| SyncError::Task(e.to_string()))??;
    Ok(stats)
}

/// Removes a view's files, leaving its root directory for whoever made it.
pub(crate) async fn dematerialize(config: &ViewConfig) -> Result<u64, AppError> {
    let paths = [config.movies_path(), config.shows_path()];
    let removed = tokio::task::spawn_blocking(move || {
        paths
            .iter()
            .map(|path| remove_symlink_tree(path))
            .sum::<Result<u64, SyncError>>()
    })
    .await
    .map_err(|e| SyncError::Task(e.to_string()))??;
    Ok(removed)
}

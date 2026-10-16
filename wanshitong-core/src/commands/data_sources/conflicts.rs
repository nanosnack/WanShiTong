//! What a data source, new or restored, may clash with: a view's root, and
//! another source's name.

use crate::{
    CoreError, CoreErrorContext, commands::views::view_roots, error::UniqueViolation,
    validation::paths_overlap,
};
use sqlx::SqliteTransaction;
use std::path::Path;

/// Ingest would otherwise measure a view's own symlinks.
pub(super) async fn check_no_view_overlap(
    txn: &mut SqliteTransaction<'_>,
    root_path: &Path,
) -> Result<(), CoreError> {
    match view_roots(txn)
        .await?
        .into_iter()
        .find(|(_, view_root)| paths_overlap(root_path, Path::new(view_root)))
    {
        Some((view, _)) => Err(CoreErrorContext::DataSourceOverlapsView {
            root_path: root_path.to_path_buf(),
            view,
        }
        .into()),
        None => Ok(()),
    }
}

/// Names a clash with another undeleted data source's name.
pub(super) fn name_error(e: sqlx::Error, name: String) -> CoreError {
    if let Some(UniqueViolation::DataSourceName) = UniqueViolation::detect(&e) {
        CoreErrorContext::DataSourceNameExists(name).into()
    } else {
        CoreErrorContext::from(e).into()
    }
}

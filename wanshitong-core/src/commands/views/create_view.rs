use super::view_roots;
use crate::time::stored_timestamp;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{View, ViewConfig, ViewRowId, ViewUuid},
    error::UniqueViolation,
    time::unix_now_i64,
    validation::{is_normal_absolute_path, paths_overlap, validate_name},
};
use std::path::Path;
use uuid::Uuid;

/// Creates a view. It stays empty until it is materialized.
///
/// A view's root may not overlap any data source, or ingest would measure the
/// view's own symlinks, nor any other view, or one view's sync would delete the
/// other's links.
pub async fn create_view(
    ctx: &CoreContext,
    name: impl AsRef<str>,
    config: ViewConfig,
    auto_refresh: bool,
) -> Result<View, CoreError> {
    let name = name.as_ref().trim().to_string();
    validate_name(&name).map_err(CoreErrorContext::InvalidViewName)?;
    let root_path = config.root_path();
    if !is_normal_absolute_path(root_path) {
        error_stack::bail!(CoreErrorContext::InvalidViewRootPath(
            root_path.to_path_buf()
        ));
    }
    let kind = config.kind().as_db_str();

    let uuid = ViewUuid(Uuid::new_v4());
    let created_at = unix_now_i64();

    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;

    let data_sources = sqlx::query!(
        r#"
        SELECT ds.name, fs.root_path
        FROM data_sources ds
        JOIN fs_data_sources fs
            ON fs.id = ds.id
        WHERE ds.deleted_at IS NULL
        "#
    )
    .fetch_all(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;
    if let Some(data_source) = data_sources
        .into_iter()
        .find(|ds| paths_overlap(root_path, Path::new(&ds.root_path)))
    {
        error_stack::bail!(CoreErrorContext::ViewOverlapsDataSource {
            view_root_path: root_path.to_path_buf(),
            data_source: data_source.name,
        });
    }

    if let Some((view, _)) = view_roots(&mut txn)
        .await?
        .into_iter()
        .find(|(_, view_root)| paths_overlap(root_path, Path::new(view_root)))
    {
        error_stack::bail!(CoreErrorContext::ViewOverlapsView {
            view_root_path: root_path.to_path_buf(),
            view,
        });
    }

    let result: Result<ViewRowId, _> = sqlx::query_scalar!(
        r#"
        INSERT INTO views (uuid, name, kind, created_at, auto_refresh)
        VALUES (?, ?, ?, ?, ?)
        RETURNING id AS "id: _"
        "#,
        uuid,
        name,
        kind,
        created_at,
        auto_refresh
    )
    .fetch_one(&mut *txn)
    .await;

    let row_id = result.map_err(|e| name_error(e, &name))?;

    let root_path_str = root_path.to_string_lossy();
    match &config {
        ViewConfig::Jellyfin(_) => sqlx::query!(
            "INSERT INTO jellyfin_views (id, root_path) VALUES (?, ?)",
            row_id,
            root_path_str
        ),
        ViewConfig::Plex(_) => sqlx::query!(
            "INSERT INTO plex_views (id, root_path) VALUES (?, ?)",
            row_id,
            root_path_str
        ),
    }
    .execute(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;

    txn.commit().await.map_err(CoreErrorContext::from)?;

    Ok(View {
        uuid,
        name,
        created_at: stored_timestamp(created_at),
        config,
        auto_refresh,
        latest_materialization: None,
    })
}

/// Names a clash with another view's name.
fn name_error(e: sqlx::Error, name: &str) -> CoreError {
    if let Some(UniqueViolation::ViewName) = UniqueViolation::detect(&e) {
        CoreErrorContext::ViewNameExists(name.to_string()).into()
    } else {
        CoreErrorContext::from(e).into()
    }
}

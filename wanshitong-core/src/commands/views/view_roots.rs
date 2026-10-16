use crate::{CoreError, CoreErrorContext};
use sqlx::SqliteTransaction;

/// Every view's name and root path.
pub(in crate::commands) async fn view_roots(
    txn: &mut SqliteTransaction<'_>,
) -> Result<Vec<(String, String)>, CoreError> {
    let views = sqlx::query!(
        r#"
        SELECT v.name, COALESCE(jv.root_path, pv.root_path) AS "root_path!: String"
        FROM views v
        LEFT JOIN jellyfin_views jv
            ON jv.id = v.id
        LEFT JOIN plex_views pv
            ON pv.id = v.id
        "#
    )
    .fetch_all(&mut **txn)
    .await
    .map_err(CoreErrorContext::from)?;
    Ok(views
        .into_iter()
        .map(|view| (view.name, view.root_path))
        .collect())
}

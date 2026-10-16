use crate::domain::{
    InterpretationRowId, JellyfinView, PlexView, View, ViewConfig, ViewMaterialization,
    ViewSyncStats, ViewUuid,
};
use crate::time::stored_timestamp;
use crate::{CoreContext, CoreError, CoreErrorContext};

/// Views with their latest materialization, optionally only the one with
/// `uuid`.
pub(super) async fn fetch_views(
    ctx: &CoreContext,
    uuid: Option<ViewUuid>,
) -> Result<Vec<View>, CoreError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            v.uuid AS "uuid!: ViewUuid",
            v.name,
            v.created_at,
            v.auto_refresh AS "auto_refresh: bool",
            v.kind,
            jv.root_path AS "jellyfin_root_path?",
            pv.root_path AS "plex_root_path?",

            vm.interpretation_id AS "interpretation_id?: InterpretationRowId",
            vm.started_at AS "started_at?",
            vm.stopped_at AS "stopped_at?",
            vm.success AS "success?",
            vm.links_created AS "links_created?",
            vm.links_updated AS "links_updated?",
            vm.links_removed AS "links_removed?",
            vm.error AS "error?"
        FROM views v
        LEFT JOIN jellyfin_views jv
            ON jv.id = v.id
        LEFT JOIN plex_views pv
            ON pv.id = v.id
        LEFT JOIN view_materializations vm
            ON vm.id = (
                SELECT MAX(id)
                FROM view_materializations
                WHERE view_id = v.id
            )
        WHERE ?1 IS NULL OR v.uuid = ?1
        ORDER BY v.created_at, v.id
        "#,
        uuid,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;

    let views = rows
        .into_iter()
        .map(|row| {
            let latest_materialization = match (row.started_at, row.stopped_at, row.success) {
                (Some(started_at), Some(stopped_at), Some(success)) => {
                    let result = if success != 0 {
                        Ok(ViewSyncStats {
                            links_created: row.links_created.unwrap_or_default() as u64,
                            links_updated: row.links_updated.unwrap_or_default() as u64,
                            links_removed: row.links_removed.unwrap_or_default() as u64,
                        })
                    } else {
                        Err(row.error.unwrap_or_default())
                    };
                    Some(ViewMaterialization {
                        interpretation_id: row.interpretation_id,
                        started_at: stored_timestamp(started_at),
                        stopped_at: stored_timestamp(stopped_at),
                        result,
                    })
                }
                _ => None,
            };
            let config = match (
                row.kind.as_str(),
                row.jellyfin_root_path,
                row.plex_root_path,
            ) {
                ("jellyfin", Some(root_path), _) => ViewConfig::Jellyfin(JellyfinView {
                    root_path: root_path.into(),
                }),
                ("plex", _, Some(root_path)) => ViewConfig::Plex(PlexView {
                    root_path: root_path.into(),
                }),
                _ => error_stack::bail!(CoreErrorContext::UnknownViewKind(row.kind)),
            };
            Ok(View {
                uuid: row.uuid,
                name: row.name,
                created_at: stored_timestamp(row.created_at),
                config,
                auto_refresh: row.auto_refresh,
                latest_materialization,
            })
        })
        .collect::<Result<_, CoreError>>()?;

    Ok(views)
}

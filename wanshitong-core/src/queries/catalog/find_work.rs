use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{CatalogWorkKind, InterpretationUuid},
};
use uuid::Uuid;

/// What kind of work has `uuid`, and which interpretation's catalog holds
/// it, or None if no catalog has it.
pub async fn find_work(ctx: &CoreContext, uuid: Uuid) -> Result<Option<FoundWork>, CoreError> {
    let row = sqlx::query!(
        r#"
        SELECT
            kind AS "kind!: String",
            interpretation_uuid AS "interpretation_uuid!: InterpretationUuid"
        FROM (
            SELECT 'movie' AS kind, i.uuid AS interpretation_uuid
            FROM movies m
            JOIN interpretations i
                ON i.id = m.interpretation_id
            WHERE m.uuid = ?1
            UNION ALL
            SELECT 'tv_series' AS kind, i.uuid AS interpretation_uuid
            FROM tv_series s
            JOIN interpretations i
                ON i.id = s.interpretation_id
            WHERE s.uuid = ?1
        )
        LIMIT 1
        "#,
        uuid,
    )
    .fetch_optional(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;
    Ok(row.map(|row| FoundWork {
        kind: match row.kind.as_str() {
            "movie" => CatalogWorkKind::Movie,
            _ => CatalogWorkKind::TvSeries,
        },
        interpretation_uuid: row.interpretation_uuid,
    }))
}

pub struct FoundWork {
    pub kind: CatalogWorkKind,
    pub interpretation_uuid: InterpretationUuid,
}

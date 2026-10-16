use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{DataSourceUuid, InterpretationError, InterpretationUuid, PathId},
};

/// What an interpretation couldn't do, in the order it was recorded.
pub async fn list_interpretation_errors(
    ctx: &CoreContext,
    interpretation: InterpretationUuid,
) -> Result<Vec<InterpretationError>, CoreError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            ds.uuid AS "data_source?: DataSourceUuid",
            f.rel_path AS "rel_path?",
            e.subject,
            e.error
        FROM interpretation_errors e
        JOIN interpretations i ON i.id = e.interpretation_id
        LEFT JOIN file_measurements f ON f.id = e.file_id
        LEFT JOIN data_sources ds ON ds.id = f.data_source_id
        WHERE i.uuid = ?
        ORDER BY e.id
        "#,
        interpretation,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;
    Ok(rows
        .into_iter()
        .map(|row| InterpretationError {
            file: row
                .data_source
                .zip(row.rel_path)
                .map(|(data_source, rel_path)| PathId::new(data_source, rel_path)),
            subject: row.subject,
            error: row.error,
        })
        .collect())
}

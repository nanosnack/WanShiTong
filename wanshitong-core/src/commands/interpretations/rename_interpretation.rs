use crate::{
    CoreContext, CoreError, CoreErrorContext, domain::InterpretationUuid, validation::validate_name,
};

pub async fn rename_interpretation(
    ctx: &CoreContext,
    uuid: InterpretationUuid,
    name: impl AsRef<str>,
) -> Result<(), CoreError> {
    let name = name.as_ref().trim().to_string();
    validate_name(&name).map_err(CoreErrorContext::InvalidInterpretationName)?;
    let result = sqlx::query!(
        "UPDATE interpretations SET name = ? WHERE uuid = ?",
        name,
        uuid,
    )
    .execute(&ctx.writer)
    .await
    .map_err(CoreErrorContext::from)?;
    if result.rows_affected() == 0 {
        error_stack::bail!(CoreErrorContext::InterpretationNotFound(uuid));
    }
    Ok(())
}

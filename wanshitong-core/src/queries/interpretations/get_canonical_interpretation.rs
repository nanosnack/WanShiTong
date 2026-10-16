use super::interpretation_rows::load_interpretations;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{Interpretation, InterpretationRowId},
};

/// `None` until the first catalog has been generated.
pub async fn get_canonical_interpretation_id(
    ctx: &CoreContext,
) -> Result<Option<InterpretationRowId>, CoreError> {
    let id = sqlx::query_scalar!(
        r#"
        SELECT canonical_interpretation_id AS "id?: _"
        FROM system_state
        WHERE id = 1
        "#
    )
    .fetch_one(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;
    Ok(id)
}

/// `None` until the first catalog has been generated.
pub async fn get_canonical_interpretation(
    ctx: &CoreContext,
) -> Result<Option<Interpretation>, CoreError> {
    let mut txn = ctx.reader.begin().await.map_err(CoreErrorContext::from)?;
    let interpretation = load_interpretations(&mut txn, None, true, None)
        .await?
        .pop();
    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(interpretation)
}

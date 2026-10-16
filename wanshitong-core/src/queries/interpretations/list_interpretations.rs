use super::interpretation_rows::load_interpretations;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{ChangesetUuid, Interpretation},
};

/// All interpretations, newest first.
pub async fn list_interpretations(ctx: &CoreContext) -> Result<Vec<Interpretation>, CoreError> {
    let mut txn = ctx.reader.begin().await.map_err(CoreErrorContext::from)?;
    let interpretations = load_interpretations(&mut txn, None, false, None).await?;
    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(interpretations)
}

/// A changeset's interpretations, newest first: those that include its
/// changes, made to preview or to accept it.
pub async fn list_changeset_interpretations(
    ctx: &CoreContext,
    changeset: ChangesetUuid,
) -> Result<Vec<Interpretation>, CoreError> {
    let mut txn = ctx.reader.begin().await.map_err(CoreErrorContext::from)?;
    let interpretations = load_interpretations(&mut txn, None, false, Some(changeset)).await?;
    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(interpretations)
}

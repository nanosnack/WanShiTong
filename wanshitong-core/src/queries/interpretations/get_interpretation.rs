use super::interpretation_rows::load_interpretations;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{Interpretation, InterpretationUuid},
};

pub async fn get_interpretation(
    ctx: &CoreContext,
    uuid: InterpretationUuid,
) -> Result<Option<Interpretation>, CoreError> {
    let mut txn = ctx.reader.begin().await.map_err(CoreErrorContext::from)?;
    let interpretation = load_interpretations(&mut txn, Some(uuid), false, None)
        .await?
        .pop();
    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(interpretation)
}

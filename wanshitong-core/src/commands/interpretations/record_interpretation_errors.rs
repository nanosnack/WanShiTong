use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{FileMeasurementRowId, InterpretationRowId},
};

/// Something an interpretation couldn't do, to record with it. See
/// [`crate::domain::InterpretationError`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewInterpretationError {
    pub file_id: Option<FileMeasurementRowId>,
    pub subject: String,
    pub error: String,
}

/// Records what an interpretation couldn't do, in the order given.
pub async fn record_interpretation_errors(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
    errors: &[NewInterpretationError],
) -> Result<(), CoreError> {
    if errors.is_empty() {
        return Ok(());
    }
    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;
    for NewInterpretationError {
        file_id,
        subject,
        error,
    } in errors
    {
        sqlx::query!(
            "INSERT INTO interpretation_errors (interpretation_id, file_id, subject, error)
             VALUES (?, ?, ?, ?)",
            interpretation_id,
            file_id,
            subject,
            error,
        )
        .execute(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?;
    }
    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(())
}

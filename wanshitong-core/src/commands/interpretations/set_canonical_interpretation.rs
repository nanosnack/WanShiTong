use crate::domain::ChangesetState;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{ChangesetUuid, InterpretationRowId, InterpretationUuid},
};

/// Makes an interpretation's catalog the canonical one. Only an
/// interpretation of accepted assertions can be: one without a changeset, or
/// of a changeset that was accepted. Another changeset's interpretation
/// previews changes that haven't happened, or never will.
pub async fn set_canonical_interpretation(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
) -> Result<(), CoreError> {
    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;
    let row = sqlx::query!(
        r#"
        SELECT
            i.uuid AS "uuid!: InterpretationUuid",
            c.uuid AS "changeset_uuid?: ChangesetUuid",
            c.state AS "changeset_state?"
        FROM interpretations i
        LEFT JOIN changesets c
            ON c.id = i.changeset_id
        WHERE i.id = ?
        "#,
        interpretation_id,
    )
    .fetch_one(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;
    if let (Some(changeset), Some(state)) = (row.changeset_uuid, row.changeset_state)
        && ChangesetState::from_db_str(&state)? != ChangesetState::Accepted
    {
        error_stack::bail!(CoreErrorContext::InterpretationOfUnacceptedChangeset {
            interpretation: row.uuid,
            changeset,
        });
    }
    sqlx::query!(
        r#"UPDATE system_state SET canonical_interpretation_id = ?"#,
        interpretation_id
    )
    .execute(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;
    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(())
}

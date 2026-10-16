use crate::{
    CoreContext, CoreError, CoreErrorContext,
    changesets::{find_changeset, select_interpretation_assertions},
    current_build,
    domain::{
        ChangesetState, ChangesetUuid, DataSourceRowId, DataSourceUuid, IngestRowId, IngestUuid,
        InterpretationRowId, InterpretationUuid,
    },
    time::unix_now_i64,
    validation::validate_name,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct NewInterpretation {
    pub row_id: InterpretationRowId,
    pub uuid: InterpretationUuid,
}

/// Creates an interpretation of the given successful ingests, one per data
/// source, and the accepted assertions, by this build of WST. Its catalog is recorded separately,
/// then it's marked finished with
/// [`finish_interpretation`](super::finish_interpretation).
pub async fn create_interpretation(
    ctx: &CoreContext,
    name: impl AsRef<str>,
    sources: &[(DataSourceUuid, IngestUuid)],
) -> Result<NewInterpretation, CoreError> {
    insert_interpretation(ctx, name.as_ref(), sources, None).await
}

/// Creates an interpretation like [`create_interpretation`], of a changeset:
/// what accepting an open one would make of the accepted assertions, to
/// compare with the canonical catalog, or what accepting an accepted one
/// made. A discarded changeset's changes never happen, so it has none.
pub async fn create_changeset_interpretation(
    ctx: &CoreContext,
    name: impl AsRef<str>,
    sources: &[(DataSourceUuid, IngestUuid)],
    changeset: ChangesetUuid,
) -> Result<NewInterpretation, CoreError> {
    insert_interpretation(ctx, name.as_ref(), sources, Some(changeset)).await
}

async fn insert_interpretation(
    ctx: &CoreContext,
    name: &str,
    sources: &[(DataSourceUuid, IngestUuid)],
    changeset: Option<ChangesetUuid>,
) -> Result<NewInterpretation, CoreError> {
    let name = name.trim().to_string();
    validate_name(&name).map_err(CoreErrorContext::InvalidInterpretationName)?;
    let uuid = InterpretationUuid(Uuid::new_v4());
    let started_at = unix_now_i64();
    let build = current_build();

    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;
    let changeset_id = match changeset {
        Some(uuid) => {
            let changeset = find_changeset(&mut txn, uuid).await?;
            if changeset.state == ChangesetState::Discarded {
                error_stack::bail!(CoreErrorContext::ChangesetNotOpen(uuid));
            }
            Some(changeset.id)
        }
        None => None,
    };
    let row_id: InterpretationRowId = sqlx::query_scalar!(
        r#"
        INSERT INTO interpretations (uuid, name, started_at, changeset_id, build)
        VALUES (?, ?, ?, ?, ?)
        RETURNING id AS "id: _"
        "#,
        uuid,
        name,
        started_at,
        changeset_id,
        build,
    )
    .fetch_one(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;
    for assertion_id in select_interpretation_assertions(&mut txn, changeset_id).await? {
        sqlx::query!(
            "INSERT INTO interpretation_assertions (interpretation_id, assertion_id) VALUES (?, ?)",
            row_id,
            assertion_id,
        )
        .execute(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?;
    }
    for &(data_source_uuid, ingest_uuid) in sources {
        let record = sqlx::query!(
            r#"
            SELECT
                i.id AS "ingest_id!: IngestRowId",
                ds.id AS "data_source_id!: DataSourceRowId",
                ds.deleted_at,
                i.success
            FROM ingests i
            JOIN data_sources ds
                ON ds.id = i.data_source_id
            WHERE ds.uuid = ? AND i.uuid = ?
            "#,
            data_source_uuid,
            ingest_uuid,
        )
        .fetch_optional(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?
        .ok_or(CoreErrorContext::DataSourceAndIngestNotFound(
            data_source_uuid,
            ingest_uuid,
        ))?;
        if record.deleted_at.is_some() {
            error_stack::bail!(CoreErrorContext::DataSourceDeleted(data_source_uuid));
        }
        // A running ingest's files are committed as they're probed, so
        // interpreting one would derive a catalog from only some of them.
        if record.success != Some(1) {
            error_stack::bail!(CoreErrorContext::IngestNotSucceeded(ingest_uuid));
        }
        sqlx::query!(
            "
            INSERT INTO interpretation_ingests
                (interpretation_id, data_source_id, ingest_id)
            VALUES (?, ?, ?)",
            row_id,
            record.data_source_id,
            record.ingest_id,
        )
        .execute(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?;
    }
    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(NewInterpretation { row_id, uuid })
}

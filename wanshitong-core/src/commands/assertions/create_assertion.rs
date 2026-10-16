use crate::domain::PathId;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    changesets::find_open_changeset,
    domain::{AssertionUuid, ChangesetRowId, ChangesetUuid, ClaimPredicate},
    json,
    queries::disc_index_of,
    time::unix_now_i64,
};
use sqlx::SqliteConnection;
use uuid::Uuid;

pub struct NewAssertion {
    /// The files the claim is about, by data source and path.
    pub files: Vec<PathId>,
    pub claim: ClaimPredicate,
    /// Why it's made, shown in audit logs.
    pub rationale: Option<String>,
    /// How sure its author is, from 0 to 1, for ordering reviews.
    pub confidence: Option<f32>,
}

/// Asserts a claim about a set of files in an open changeset. It's pending
/// until the changeset is accepted, when it replaces any older assertion of
/// the same kind about each file.
pub async fn create_assertion(
    ctx: &CoreContext,
    changeset: ChangesetUuid,
    assertion: NewAssertion,
) -> Result<AssertionUuid, CoreError> {
    let uuids = create_assertions(ctx, changeset, vec![assertion]).await?;
    Ok(uuids[0])
}

/// Makes several assertions like [`create_assertion`], all or none of them.
pub async fn create_assertions(
    ctx: &CoreContext,
    changeset: ChangesetUuid,
    assertions: Vec<NewAssertion>,
) -> Result<Vec<AssertionUuid>, CoreError> {
    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;
    let changeset_id = find_open_changeset(&mut txn, changeset).await?;
    let mut uuids = Vec::with_capacity(assertions.len());
    for assertion in assertions {
        uuids.push(insert_assertion(&mut txn, changeset_id, assertion).await?);
    }
    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(uuids)
}

async fn insert_assertion(
    txn: &mut SqliteConnection,
    changeset_id: ChangesetRowId,
    assertion: NewAssertion,
) -> Result<AssertionUuid, CoreError> {
    let NewAssertion {
        files,
        claim,
        rationale,
        confidence,
    } = assertion;
    if files.is_empty() {
        error_stack::bail!(CoreErrorContext::AssertionWithoutFiles);
    }
    if let Some(confidence) = confidence
        && !(0.0..=1.0).contains(&confidence)
    {
        error_stack::bail!(CoreErrorContext::InvalidConfidence(confidence));
    }
    let rationale = rationale
        .map(|rationale| rationale.trim().to_string())
        .filter(|rationale| !rationale.is_empty());
    let uuid = AssertionUuid(Uuid::new_v4());
    let now = unix_now_i64();
    let data = json::encode(&claim);

    let group_id = sqlx::query_scalar!(
        r#"
        INSERT INTO assertion_groups (uuid, created_at, changeset_id, rationale, confidence)
        VALUES (?, ?, ?, ?, ?)
        RETURNING id AS "id!"
        "#,
        uuid,
        now,
        changeset_id,
        rationale,
        confidence,
    )
    .fetch_one(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;

    for PathId {
        data_source,
        rel_path,
    } in files
    {
        let rel_path = rel_path.to_string_lossy().into_owned();
        let source = sqlx::query!(
            r#"SELECT id AS "id!", deleted_at FROM data_sources WHERE uuid = ?"#,
            data_source
        )
        .fetch_optional(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?
        .ok_or(CoreErrorContext::DataSourceNotFound(data_source))?;
        if source.deleted_at.is_some() {
            error_stack::bail!(CoreErrorContext::DataSourceDeleted(data_source));
        }
        // A disc is named by its index file, which stands for its other files.
        if let Some(index) = disc_index_of(&mut *txn, source.id, &rel_path).await? {
            error_stack::bail!(CoreErrorContext::PartOfDisc {
                file: rel_path,
                index: index.display().to_string(),
            });
        }

        // Each file's row has a UUID of its own, but it's the group's that
        // names the assertion.
        let row_uuid = Uuid::new_v4();
        sqlx::query!(
            r#"
            INSERT INTO assertions
                (uuid, group_id, data_source_id, rel_path, claim, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            row_uuid,
            group_id,
            source.id,
            rel_path,
            data,
            now,
        )
        .execute(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?;
    }
    Ok(uuid)
}

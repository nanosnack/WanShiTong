//! What changesets share between commands and queries: finding one, and
//! which assertions an interpretation includes.

use crate::{
    CoreError, CoreErrorContext,
    domain::{ChangesetRowId, ChangesetState, ChangesetUuid, ClaimKind, ClaimPredicate},
    json,
};
use sqlx::SqliteConnection;
use std::collections::HashSet;

pub(crate) struct ChangesetRow {
    pub id: ChangesetRowId,
    pub state: ChangesetState,
}

pub(crate) async fn find_changeset(
    conn: &mut SqliteConnection,
    uuid: ChangesetUuid,
) -> Result<ChangesetRow, CoreError> {
    let row = sqlx::query!(
        r#"SELECT id AS "id!: ChangesetRowId", state FROM changesets WHERE uuid = ?"#,
        uuid,
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(CoreErrorContext::from)?
    .ok_or(CoreErrorContext::ChangesetNotFound(uuid))?;
    Ok(ChangesetRow {
        id: row.id,
        state: ChangesetState::from_db_str(&row.state)?,
    })
}

/// An open changeset, or an error saying it isn't one.
pub(crate) async fn find_open_changeset(
    conn: &mut SqliteConnection,
    uuid: ChangesetUuid,
) -> Result<ChangesetRowId, CoreError> {
    let changeset = find_changeset(conn, uuid).await?;
    match changeset.state {
        ChangesetState::Open => Ok(changeset.id),
        _ => Err(CoreErrorContext::ChangesetNotOpen(uuid).into()),
    }
}

/// The assertion rows, one per file, that a new interpretation includes:
/// every accepted assertion still in effect about an undeleted data source.
/// An open changeset's interpretation includes what accepting the changeset
/// would change: its pending assertions, but not the accepted ones it
/// retracts, nor those its assertions would replace, which are of the same
/// kind about the same file. Sorted, to compare with an interpretation's.
pub(crate) async fn select_interpretation_assertions(
    conn: &mut SqliteConnection,
    changeset: Option<ChangesetRowId>,
) -> Result<Vec<i64>, CoreError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            a.id AS "id!",
            a.group_id AS "group_id!",
            a.data_source_id AS "data_source_id!",
            a.rel_path,
            a.claim,
            g.changeset_id AS "changeset_id: ChangesetRowId"
        FROM assertions a
        JOIN assertion_groups g
            ON g.id = a.group_id
        JOIN changesets c
            ON c.id = g.changeset_id
        JOIN data_sources ds
            ON ds.id = a.data_source_id
        WHERE
            a.retracted_at IS NULL
            AND ds.deleted_at IS NULL
            AND (c.state = 'accepted' OR c.id = ?1)
        ORDER BY a.id
        "#,
        changeset,
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(CoreErrorContext::from)?;
    let Some(changeset) = changeset else {
        return Ok(rows.into_iter().map(|row| row.id).collect());
    };

    let retracted: HashSet<i64> = sqlx::query_scalar!(
        r#"
        SELECT assertion_group_id AS "group_id!"
        FROM changeset_retractions
        WHERE changeset_id = ?
        "#,
        changeset,
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(CoreErrorContext::from)?
    .into_iter()
    .collect();
    let mut replaced = HashSet::<(i64, String, ClaimKind)>::new();
    for row in rows.iter().filter(|row| row.changeset_id == changeset) {
        let claim: ClaimPredicate = json::decode(&row.claim)?;
        replaced.insert((row.data_source_id, row.rel_path.clone(), claim.kind()));
    }
    let mut ids = Vec::with_capacity(rows.len());
    for row in rows {
        let keep = if row.changeset_id == changeset {
            true
        } else {
            let claim: ClaimPredicate = json::decode(&row.claim)?;
            !retracted.contains(&row.group_id)
                && !replaced.contains(&(row.data_source_id, row.rel_path, claim.kind()))
        };
        if keep {
            ids.push(row.id);
        }
    }
    Ok(ids)
}

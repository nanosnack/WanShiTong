use crate::time::stored_timestamp;
use crate::{
    CoreError, CoreErrorContext,
    domain::{
        ChangesetState, ChangesetUuid, DataSourceUuid, IngestUuid, Interpretation,
        InterpretationChangeset, InterpretationIngest, InterpretationRowId, InterpretationUuid,
    },
};
use sqlx::SqliteConnection;

/// Every interpretation, or just the one with `uuid`, or just the canonical
/// one, or just those of `changeset`.
pub(super) async fn load_interpretations(
    conn: &mut SqliteConnection,
    uuid: Option<InterpretationUuid>,
    only_canonical: bool,
    changeset: Option<ChangesetUuid>,
) -> Result<Vec<Interpretation>, CoreError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            i.id AS "id!: InterpretationRowId",
            i.uuid AS "uuid!: InterpretationUuid",
            i.name,
            i.started_at,
            i.stopped_at,
            i.error,
            COALESCE(i.id = ss.canonical_interpretation_id, 0) AS "is_canonical!: bool",
            c.uuid AS "changeset_uuid?: ChangesetUuid",
            c.author AS "changeset_author?",
            c.description AS "changeset_description?",
            c.state AS "changeset_state?",
            (SELECT COUNT(*) FROM movies m WHERE m.interpretation_id = i.id) AS "movie_count!: i64",
            (
                SELECT COUNT(*)
                FROM tv_episodes e
                JOIN tv_seasons s ON s.id = e.season_id
                JOIN tv_series ts ON ts.id = s.series_id
                WHERE ts.interpretation_id = i.id
            ) AS "episode_count!: i64",
            (
                SELECT COUNT(*) FROM interpretation_errors e WHERE e.interpretation_id = i.id
            ) AS "error_count!: i64"
        FROM interpretations i
        JOIN system_state ss
        LEFT JOIN changesets c
            ON c.id = i.changeset_id
        WHERE
            (?1 IS NULL OR i.uuid = ?1)
            AND (NOT ?2 OR i.id IS ss.canonical_interpretation_id)
            AND (?3 IS NULL OR c.uuid = ?3)
        ORDER BY i.started_at DESC, i.id DESC
        "#,
        uuid,
        only_canonical,
        changeset,
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(CoreErrorContext::from)?;

    let mut interpretations = Vec::with_capacity(rows.len());
    for row in rows {
        let ingests = sqlx::query!(
            r#"
            SELECT
                ds.uuid AS "data_source_uuid!: DataSourceUuid",
                ds.name AS data_source_name,
                i.uuid AS "ingest_uuid!: IngestUuid",
                i.started_at AS ingest_started_at
            FROM interpretation_ingests ii
            JOIN data_sources ds
                ON ds.id = ii.data_source_id
            JOIN ingests i
                ON i.id = ii.ingest_id
            WHERE ii.interpretation_id = ?
            ORDER BY ds.name
            "#,
            row.id,
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(CoreErrorContext::from)?
        .into_iter()
        .map(|ingest| InterpretationIngest {
            data_source_uuid: ingest.data_source_uuid,
            data_source_name: ingest.data_source_name,
            ingest_uuid: ingest.ingest_uuid,
            ingest_started_at: stored_timestamp(ingest.ingest_started_at),
        })
        .collect();

        let changeset = match (
            row.changeset_uuid,
            row.changeset_author,
            row.changeset_description,
            row.changeset_state,
        ) {
            (Some(uuid), Some(author), Some(description), Some(state)) => {
                Some(InterpretationChangeset {
                    uuid,
                    author,
                    description,
                    state: ChangesetState::from_db_str(&state)?,
                })
            }
            _ => None,
        };

        interpretations.push(Interpretation {
            id: row.id,
            uuid: row.uuid,
            name: row.name,
            started_at: stored_timestamp(row.started_at),
            stopped_at: row.stopped_at.map(stored_timestamp),
            error: row.error,
            is_canonical: row.is_canonical,
            movie_count: row.movie_count as u64,
            episode_count: row.episode_count as u64,
            error_count: row.error_count as u64,
            ingests,
            changeset,
        });
    }
    Ok(interpretations)
}

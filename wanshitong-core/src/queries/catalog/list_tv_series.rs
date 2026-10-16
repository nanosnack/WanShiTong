use crate::domain::{InterpretationRowId, TvSeriesRowId, TvSeriesSummary, TvSeriesUuid};
use crate::{CoreContext, CoreError, CoreErrorContext};
use serde::{Deserialize, Serialize};

/// The TV series, by name. A catalog holds only identified works; the files of
/// the rest are among its unidentified videos.
pub async fn list_tv_series(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
    query: &TvSeriesSearchQuery,
) -> Result<TvSeriesSummaryList, CoreError> {
    let TvSeriesSearchQuery {
        name,
        cursor,
        limit,
    } = query;
    let (cursor_sort_name, cursor_row_id) = match cursor {
        Some(TvSeriesCursor { sort_name, row_id }) => (Some(sort_name), Some(row_id)),
        None => (None, None),
    };
    let query_limit = *limit as i64 + 1;
    let mut rows = sqlx::query!(
        r#"
        SELECT
            s.id AS "id!: TvSeriesRowId",
            s.uuid AS "uuid!: TvSeriesUuid",
            s.name,
            s.sort_name AS "sort_name!",
            s.tmdb_id,
            s.tmdb_poster_path,
            (
                SELECT count(*)
                FROM tv_seasons
                WHERE series_id = s.id AND number IS NOT NULL
            ) AS "season_count!: i64",
            (
                SELECT count(*)
                FROM tv_episodes e
                JOIN tv_seasons se
                    ON se.id = e.season_id
                WHERE se.series_id = s.id
            ) AS "episode_count!: i64"
        FROM tv_series s
        WHERE
            s.interpretation_id = ?1
            AND (?2 IS NULL OR lower(s.name) LIKE '%' || lower(?2) || '%')
            AND (
                ?3 IS NULL
                OR (
                    s.sort_name > ?3
                    OR (s.sort_name = ?3 AND s.id >= ?4)
                )
            )
        ORDER BY s.sort_name, s.id
        LIMIT ?5;
        "#,
        interpretation_id,
        name,
        cursor_sort_name,
        cursor_row_id,
        query_limit,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;

    let next_cursor = if rows.len() == query_limit as usize {
        rows.last().map(|row| TvSeriesCursor {
            sort_name: row.sort_name.clone(),
            row_id: row.id,
        })
    } else {
        None
    };
    rows.truncate(*limit);
    let series = rows
        .into_iter()
        .map(|row| TvSeriesSummary {
            id: row.id,
            uuid: row.uuid,
            name: row.name,
            tmdb_id: row.tmdb_id,
            tmdb_poster_path: row.tmdb_poster_path,
            season_count: row.season_count,
            episode_count: row.episode_count,
        })
        .collect();

    Ok(TvSeriesSummaryList {
        series,
        next_cursor,
    })
}

pub struct TvSeriesSearchQuery {
    pub name: Option<String>,
    pub cursor: Option<TvSeriesCursor>,
    pub limit: usize,
}

impl Default for TvSeriesSearchQuery {
    fn default() -> Self {
        Self {
            name: Default::default(),
            cursor: Default::default(),
            limit: 50,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct TvSeriesCursor {
    pub sort_name: String,
    pub row_id: TvSeriesRowId,
}

pub struct TvSeriesSummaryList {
    pub series: Vec<TvSeriesSummary>,
    pub next_cursor: Option<TvSeriesCursor>,
}

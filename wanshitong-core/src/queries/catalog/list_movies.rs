use crate::domain::{InterpretationRowId, MovieRowId, MovieSummary, MovieUuid};
use crate::{CoreContext, CoreError, CoreErrorContext};
use serde::{Deserialize, Serialize};

/// The movies, by title. A catalog holds only identified works; the files of
/// the rest are among its unidentified videos.
pub async fn list_movies(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
    query: &MovieSearchQuery,
) -> Result<MovieSummaryList, CoreError> {
    let MovieSearchQuery {
        title,
        with_extras,
        cursor,
        limit,
    } = query;
    let (cursor_sort_title, cursor_row_id) = match cursor {
        Some(MovieCursor { sort_title, row_id }) => (Some(sort_title), Some(row_id)),
        None => (None, None),
    };
    let query_limit = *limit as i64 + 1;
    let mut rows = sqlx::query!(
        r#"
        SELECT
            id AS "id!: MovieRowId",
            uuid AS "uuid: MovieUuid",
            title,
            sort_title AS "sort_title!",
            release_year,
            tmdb_id,
            tmdb_poster_path
        FROM movies
        WHERE
            interpretation_id = ?1
            AND (?2 IS NULL OR lower(title) LIKE '%' || lower(?2) || '%')
            AND (
                NOT ?6
                OR EXISTS (
                    SELECT 1
                    FROM movie_files mf
                    WHERE mf.movie_id = movies.id AND mf.role = 'extra'
                )
            )
            AND (
                ?3 IS NULL
                OR (
                    sort_title > ?3
                    OR (sort_title = ?3 AND id >= ?4)
                )
            )
        ORDER BY sort_title, id
        LIMIT ?5;
        "#,
        interpretation_id,
        title,
        cursor_sort_title,
        cursor_row_id,
        query_limit,
        with_extras,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;

    let next_cursor = if rows.len() == query_limit as usize {
        rows.last().map(|row| MovieCursor {
            sort_title: row.sort_title.clone(),
            row_id: row.id,
        })
    } else {
        None
    };
    rows.truncate(*limit);
    let movies = rows
        .into_iter()
        .map(|row| MovieSummary {
            id: row.id,
            uuid: row.uuid,
            title: row.title,
            release_year: row.release_year,
            tmdb_id: row.tmdb_id,
            tmdb_poster_path: row.tmdb_poster_path,
        })
        .collect();

    Ok(MovieSummaryList {
        movies,
        next_cursor,
    })
}

pub struct MovieSearchQuery {
    pub title: Option<String>,
    /// Only movies with at least one extra.
    pub with_extras: bool,
    pub cursor: Option<MovieCursor>,
    pub limit: usize,
}

impl Default for MovieSearchQuery {
    fn default() -> Self {
        Self {
            title: Default::default(),
            with_extras: false,
            cursor: Default::default(),
            limit: 50,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct MovieCursor {
    pub sort_title: String,
    pub row_id: MovieRowId,
}

pub struct MovieSummaryList {
    pub movies: Vec<MovieSummary>,
    pub next_cursor: Option<MovieCursor>,
}

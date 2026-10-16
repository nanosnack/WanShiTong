use crate::domain::{FileRole, InterpretationRowId, ViewMovie, ViewMovieFile};
use crate::{CoreContext, CoreError, CoreErrorContext};
use std::path::Path;

/// Every movie in an interpretation's catalog, with the absolute paths of its
/// files, and which manifestation each main file is in.
pub async fn list_movies_for_view(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
) -> Result<Vec<ViewMovie>, CoreError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            m.id AS "id!",
            m.title,
            m.release_year,
            m.tmdb_id,
            m.imdb_id,
            mf.role,
            mf.manifestation AS "manifestation: u32",
            mf.part AS "part: u32",
            me.edition,
            fm.rel_path,
            fs.root_path,
            EXISTS (
                SELECT 1 FROM discs d WHERE d.index_file_id = fm.id
            ) AS "disc!: bool"
        FROM movies m
        JOIN movie_files mf
            ON mf.movie_id = m.id
        LEFT JOIN movie_expressions me
            ON me.id = mf.expression_id
        JOIN file_measurements fm
            ON fm.id = mf.file_id
        JOIN fs_data_sources fs
            ON fs.id = fm.data_source_id
        WHERE m.interpretation_id = ?1
        ORDER BY m.id, mf.id
        "#,
        interpretation_id,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;

    let mut movies = Vec::<ViewMovie>::new();
    let mut last_movie_id = None;
    for row in rows {
        let file = ViewMovieFile {
            role: row.role.as_deref().map(FileRole::from_db_str).transpose()?,
            manifestation: row
                .manifestation
                .map(|manifestation| manifestation as usize),
            part: row.part,
            edition: row.edition,
            disc: row.disc,
            path: Path::new(&row.root_path).join(&row.rel_path),
        };
        if last_movie_id != Some(row.id) {
            last_movie_id = Some(row.id);
            movies.push(ViewMovie {
                title: row.title,
                release_year: row.release_year.map(|year| year as u32),
                tmdb_id: row.tmdb_id.map(|id| id as u32),
                imdb_id: row.imdb_id,
                files: Vec::new(),
            });
        }
        movies.last_mut().unwrap().files.push(file);
    }
    Ok(movies)
}

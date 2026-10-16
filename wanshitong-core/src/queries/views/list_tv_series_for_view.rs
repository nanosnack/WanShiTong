use crate::domain::{InterpretationRowId, TvSeriesRowId, ViewEpisodeFile, ViewTvSeries};
use crate::{CoreContext, CoreError, CoreErrorContext};
use std::collections::BTreeMap;
use std::path::Path;

/// Every TV series in an interpretation's catalog, with the absolute paths of
/// its episodes' files and its extras.
pub async fn list_tv_series_for_view(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
) -> Result<Vec<ViewTvSeries>, CoreError> {
    let series_rows = sqlx::query!(
        r#"
        SELECT
            id AS "id!: TvSeriesRowId",
            name,
            tmdb_id,
            tvdb_id,
            imdb_id
        FROM tv_series
        WHERE interpretation_id = ?1
        "#,
        interpretation_id,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;
    let mut series: BTreeMap<i64, ViewTvSeries> = series_rows
        .into_iter()
        .map(|row| {
            let series = ViewTvSeries {
                name: row.name,
                tmdb_id: row.tmdb_id.map(|id| id as u32),
                tvdb_id: row.tvdb_id.map(|id| id as u32),
                imdb_id: row.imdb_id,
                episode_files: Vec::new(),
                extras: Vec::new(),
            };
            (row.id.0, series)
        })
        .collect();

    let episode_rows = sqlx::query!(
        r#"
        SELECT
            se.series_id,
            se.number AS "season: u32",
            e.number AS "episode!: u32",
            fm.rel_path,
            fs.root_path
        FROM tv_series s
        JOIN tv_seasons se
            ON se.series_id = s.id
        JOIN tv_episodes e
            ON e.season_id = se.id
        JOIN tv_episode_files ef
            ON ef.episode_id = e.id
        JOIN file_measurements fm
            ON fm.id = ef.file_id
        JOIN fs_data_sources fs
            ON fs.id = fm.data_source_id
        WHERE s.interpretation_id = ?1
        ORDER BY se.series_id, se.number IS NULL, se.number, e.number, fm.rel_path
        "#,
        interpretation_id,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;
    for row in episode_rows {
        if let Some(series) = series.get_mut(&row.series_id) {
            series.episode_files.push(ViewEpisodeFile {
                season: row.season,
                episode: row.episode,
                path: Path::new(&row.root_path).join(&row.rel_path),
            });
        }
    }

    let extra_rows = sqlx::query!(
        r#"
        SELECT
            xf.series_id,
            fm.rel_path,
            fs.root_path
        FROM tv_series s
        JOIN tv_series_extra_files xf
            ON xf.series_id = s.id
        JOIN file_measurements fm
            ON fm.id = xf.file_id
        JOIN fs_data_sources fs
            ON fs.id = fm.data_source_id
        -- A series' discs aren't its episodes or its extras.
        WHERE s.interpretation_id = ?1 AND xf.disc = 0
        ORDER BY xf.series_id, fm.rel_path
        "#,
        interpretation_id,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;
    for row in extra_rows {
        if let Some(series) = series.get_mut(&row.series_id) {
            series
                .extras
                .push(Path::new(&row.root_path).join(&row.rel_path));
        }
    }

    Ok(series.into_values().collect())
}

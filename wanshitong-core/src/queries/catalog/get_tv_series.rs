use crate::domain::DataSourceRef;
use crate::json;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{
        DataSourceUuid, SubjectAuditLog, TvEpisode, TvSeason, TvSeriesDetails, TvSeriesFile,
        TvSeriesRowId, TvSeriesUuid,
    },
};

pub async fn get_tv_series(
    ctx: &CoreContext,
    uuid: TvSeriesUuid,
) -> Result<Option<TvSeriesDetails>, CoreError> {
    let Some(series) = sqlx::query!(
        r#"
        SELECT
            id AS "id!: TvSeriesRowId",
            name,
            original_name,
            tmdb_id,
            imdb_id,
            tvdb_id,
            original_language,
            first_air_year,
            tagline,
            overview,
            tmdb_poster_path
        FROM tv_series
        WHERE uuid = ?
        "#,
        uuid,
    )
    .fetch_optional(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?
    else {
        return Ok(None);
    };

    // One row per episode file, or per episode without files.
    let rows = sqlx::query!(
        r#"
        SELECT
            se.number AS "season_number: u32",
            e.number AS "episode_number!: u32",
            e.title AS episode_title,
            ds.uuid AS "data_source_uuid?: DataSourceUuid",
            fm.rel_path AS "rel_path?",
            al.data AS "audit_log?"
        FROM tv_seasons se
        JOIN tv_series s
            ON s.id = se.series_id
        JOIN tv_episodes e
            ON e.season_id = se.id
        LEFT JOIN tv_episode_files ef
            ON ef.episode_id = e.id
        LEFT JOIN file_measurements fm
            ON fm.id = ef.file_id
        LEFT JOIN data_sources ds
            ON ds.id = fm.data_source_id
        LEFT JOIN audit_logs al
            ON al.file_id = fm.id
            AND al.interpretation_id = s.interpretation_id
        WHERE se.series_id = ?
        ORDER BY se.number IS NULL, se.number, e.number, fm.rel_path
        "#,
        series.id,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;

    let mut seasons = Vec::<TvSeason>::new();
    for row in rows {
        if seasons.last().is_none_or(|s| s.number != row.season_number) {
            seasons.push(TvSeason {
                number: row.season_number,
                episodes: Vec::new(),
            });
        }
        let episodes = &mut seasons.last_mut().unwrap().episodes;
        if episodes
            .last()
            .is_none_or(|e| e.number != row.episode_number)
        {
            episodes.push(TvEpisode {
                number: row.episode_number,
                title: row.episode_title,
                files: Vec::new(),
            });
        }
        let (Some(data_source_uuid), Some(rel_path)) = (row.data_source_uuid, row.rel_path) else {
            continue;
        };
        let audit_log = decode_audit_log(row.audit_log)?;
        episodes.last_mut().unwrap().files.push(TvSeriesFile {
            data_source_uuid,
            rel_path: rel_path.into(),
            audit_log,
        });
    }

    let (mut extras, mut discs) = (Vec::new(), Vec::new());
    for row in sqlx::query!(
        r#"
        SELECT
            ds.uuid AS "data_source_uuid!: DataSourceUuid",
            fm.rel_path,
            al.data AS "audit_log?",
            xf.disc AS "disc!: bool"
        FROM tv_series_extra_files xf
        JOIN tv_series s
            ON s.id = xf.series_id
        JOIN file_measurements fm
            ON fm.id = xf.file_id
        JOIN data_sources ds
            ON ds.id = fm.data_source_id
        LEFT JOIN audit_logs al
            ON al.file_id = fm.id
            AND al.interpretation_id = s.interpretation_id
        WHERE xf.series_id = ?
        ORDER BY fm.rel_path
        "#,
        series.id,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?
    {
        let file = TvSeriesFile {
            data_source_uuid: row.data_source_uuid,
            rel_path: row.rel_path.into(),
            audit_log: decode_audit_log(row.audit_log)?,
        };
        if row.disc {
            discs.push(file);
        } else {
            extras.push(file);
        }
    }

    let data_sources = sqlx::query!(
        r#"
        SELECT DISTINCT
            ds.uuid AS "uuid!: DataSourceUuid",
            ds.name AS "name!"
        FROM (
            SELECT ef.file_id
            FROM tv_seasons se
            JOIN tv_episodes e
                ON e.season_id = se.id
            JOIN tv_episode_files ef
                ON ef.episode_id = e.id
            WHERE se.series_id = ?1
            UNION
            SELECT file_id
            FROM tv_series_extra_files
            WHERE series_id = ?1
        ) f
        JOIN file_measurements fm
            ON fm.id = f.file_id
        JOIN data_sources ds
            ON ds.id = fm.data_source_id
        ORDER BY ds.name
        "#,
        series.id,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?
    .into_iter()
    .map(|row| DataSourceRef {
        uuid: row.uuid,
        name: row.name,
    })
    .collect();

    Ok(Some(TvSeriesDetails {
        uuid,
        name: series.name,
        original_name: series.original_name,
        tmdb_id: series.tmdb_id,
        imdb_id: series.imdb_id,
        tvdb_id: series.tvdb_id,
        original_language: series.original_language,
        first_air_year: series.first_air_year,
        tagline: series.tagline,
        overview: series.overview,
        tmdb_poster_path: series.tmdb_poster_path,
        data_sources,
        seasons,
        extras,
        discs,
    }))
}

/// A stored audit log, or an empty one if the file has none.
fn decode_audit_log(json: Option<String>) -> Result<SubjectAuditLog, CoreError> {
    match json {
        Some(json) => json::decode(&json),
        None => Ok(SubjectAuditLog::default()),
    }
}

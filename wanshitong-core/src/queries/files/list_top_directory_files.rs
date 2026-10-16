use crate::json;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{
        DataSourceUuid, DirectoryFile, FileEpisode, FileMeasurementRowId, FileMediaMeasurements,
        FileMovie, FileRole, FileSeriesExtra, InterpretationRowId, MovieUuid, TvSeriesUuid,
    },
};
use std::collections::HashMap;

/// The files under `dir` in a data source, e.g. a top directory, as the
/// interpretation's ingests measured them and its catalog placed them, in
/// order of their paths.
///
/// Files under a folder are a range of paths, which the files' index serves:
/// from the prefix up to it followed by the last code point, which no path
/// starts with in practice. `substr` would make it a scan.
pub async fn list_top_directory_files(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
    data_source_uuid: DataSourceUuid,
    dir: &str,
) -> Result<Vec<DirectoryFile>, CoreError> {
    let prefix = format!("{dir}/");
    let rows = sqlx::query!(
        r#"
        SELECT
            m.id AS "id!: FileMeasurementRowId",
            m.rel_path AS "rel_path!",
            m.size AS "size!",
            m.media_measurements AS "media_measurements?",
            EXISTS (
                SELECT 1
                FROM unidentified_videos uv
                WHERE uv.interpretation_id = ?1 AND uv.file_id = m.id
            ) AS "unidentified_video!: bool"
        FROM complete_file_measurements m
        JOIN interpretation_ingests ii
            ON ii.ingest_id = m.ingest_id
        WHERE
            ii.interpretation_id = ?1
            AND m.data_source_uuid = ?2
            AND m.rel_path >= ?3
            AND m.rel_path < ?3 || char(1114111)
        ORDER BY m.rel_path
        "#,
        interpretation_id,
        data_source_uuid,
        prefix,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;

    let mut movies: HashMap<FileMeasurementRowId, Vec<FileMovie>> = HashMap::new();
    for r in sqlx::query!(
        r#"
        SELECT
            mf.file_id AS "file_id!: FileMeasurementRowId",
            mo.uuid AS "uuid!: MovieUuid",
            mo.title,
            mo.release_year,
            mf.role,
            mf.manifestation AS "manifestation: u32",
            mf.part AS "part: u32"
        FROM movie_files mf
        JOIN movies mo
            ON mo.id = mf.movie_id
        JOIN complete_file_measurements m
            ON m.id = mf.file_id
        WHERE
            mo.interpretation_id = ?1
            AND m.data_source_uuid = ?2
            AND m.rel_path >= ?3
            AND m.rel_path < ?3 || char(1114111)
        ORDER BY mo.sort_title, mo.id
        "#,
        interpretation_id,
        data_source_uuid,
        prefix,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?
    {
        movies.entry(r.file_id).or_default().push(FileMovie {
            uuid: r.uuid,
            title: r.title,
            release_year: r.release_year,
            role: r.role.as_deref().map(FileRole::from_db_str).transpose()?,
            manifestation: r.manifestation.map(|manifestation| manifestation as usize),
            part: r.part,
        });
    }

    let mut episodes: HashMap<FileMeasurementRowId, Vec<FileEpisode>> = HashMap::new();
    for r in sqlx::query!(
        r#"
        SELECT
            ef.file_id AS "file_id!: FileMeasurementRowId",
            s.uuid AS "series_uuid!: TvSeriesUuid",
            s.name AS series_name,
            se.number AS "season_number: u32",
            e.number AS "episode_number!: u32",
            e.title
        FROM tv_episode_files ef
        JOIN tv_episodes e
            ON e.id = ef.episode_id
        JOIN tv_seasons se
            ON se.id = e.season_id
        JOIN tv_series s
            ON s.id = se.series_id
        JOIN complete_file_measurements m
            ON m.id = ef.file_id
        WHERE
            s.interpretation_id = ?1
            AND m.data_source_uuid = ?2
            AND m.rel_path >= ?3
            AND m.rel_path < ?3 || char(1114111)
        ORDER BY s.sort_name, s.id, se.number IS NULL, se.number, e.number
        "#,
        interpretation_id,
        data_source_uuid,
        prefix,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?
    {
        episodes.entry(r.file_id).or_default().push(FileEpisode {
            series_uuid: r.series_uuid,
            series_name: r.series_name,
            season_number: r.season_number,
            episode_number: r.episode_number,
            title: r.title,
        });
    }

    let mut series_extras: HashMap<FileMeasurementRowId, Vec<FileSeriesExtra>> = HashMap::new();
    for r in sqlx::query!(
        r#"
        SELECT
            xf.file_id AS "file_id!: FileMeasurementRowId",
            s.uuid AS "series_uuid!: TvSeriesUuid",
            s.name AS series_name,
            xf.disc AS "disc!: bool"
        FROM tv_series_extra_files xf
        JOIN tv_series s
            ON s.id = xf.series_id
        JOIN complete_file_measurements m
            ON m.id = xf.file_id
        WHERE
            s.interpretation_id = ?1
            AND m.data_source_uuid = ?2
            AND m.rel_path >= ?3
            AND m.rel_path < ?3 || char(1114111)
        ORDER BY s.sort_name, s.id
        "#,
        interpretation_id,
        data_source_uuid,
        prefix,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?
    {
        series_extras
            .entry(r.file_id)
            .or_default()
            .push(FileSeriesExtra {
                series_uuid: r.series_uuid,
                series_name: r.series_name,
                disc: r.disc,
            });
    }

    rows.into_iter()
        .map(|row| {
            let media: Option<FileMediaMeasurements> = row
                .media_measurements
                .as_deref()
                .map(json::decode)
                .transpose()?;
            Ok(DirectoryFile {
                rel_path: row.rel_path.into(),
                size: row.size,
                duration: media.as_ref().and_then(FileMediaMeasurements::duration),
                unidentified_video: row.unidentified_video,
                movies: movies.remove(&row.id).unwrap_or_default(),
                episodes: episodes.remove(&row.id).unwrap_or_default(),
                series_extras: series_extras.remove(&row.id).unwrap_or_default(),
            })
        })
        .collect()
}

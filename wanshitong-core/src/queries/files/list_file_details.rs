use crate::json;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{
        DataSourceUuid, DiscFacts, FileDetails, FileEpisode, FileMeasurementRowId, FileMovie,
        FileRole, FileSeriesExtra, InterpretationRowId, MovieUuid, PathId, TvSeriesUuid,
    },
};
use std::collections::HashMap;

/// The files at `paths`, as measured by the interpretation's ingests and
/// placed by its catalog, in the order given, leaving out any they didn't
/// see. A path given more than once is listed once.
///
/// However many paths, this is four queries: a work's evidence asks for
/// every file of the work at once, rather than one file at a time.
pub async fn list_file_details(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
    paths: &[PathId],
) -> Result<Vec<FileDetails>, CoreError> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    // SQLite takes a list as JSON: each path as [data source UUID in hex,
    // path], which the query takes apart with json_each.
    let paths_json = serde_json::to_string(
        &paths
            .iter()
            .map(|path| {
                (
                    path.data_source.0.simple().to_string(),
                    path.rel_path.to_string_lossy(),
                )
            })
            .collect::<Vec<_>>(),
    )
    .expect("paths serialize to JSON");

    // An interpretation reads one ingest of each data source, but take the
    // latest in case that changes: the first row for a path wins below.
    let rows = sqlx::query!(
        r#"
        SELECT
            m.id AS "id!: FileMeasurementRowId",
            m.data_source_uuid AS "data_source_uuid!: DataSourceUuid",
            m.data_source_name AS "data_source_name!",
            m.rel_path AS "rel_path!",
            m.size AS "size!",
            m.mtime AS "mtime!",
            m.mime_type AS "mime_type?",
            m.media_measurements AS "media_measurements?",
            al.data AS "audit_log?",
            d.kind AS "disc_kind?: String",
            d.root_path AS "disc_root?: String",
            d.size AS "disc_size?: i64",
            d.duration_ms AS "disc_duration_ms?: i64",
            (SELECT count(*) FROM disc_files df WHERE df.disc_id = d.id) AS "disc_member_count?: i64",
            ix.rel_path AS "part_of_disc?: String"
        FROM json_each(?2) path
        JOIN complete_file_measurements m
            ON m.data_source_uuid = unhex(json_extract(path.value, '$[0]'))
            AND m.rel_path = json_extract(path.value, '$[1]')
        JOIN interpretation_ingests ii
            ON ii.ingest_id = m.ingest_id
        LEFT JOIN discs d
            ON d.index_file_id = m.id
        LEFT JOIN disc_files member
            ON member.file_id = m.id
        LEFT JOIN discs od
            ON od.id = member.disc_id
        LEFT JOIN file_measurements ix
            ON ix.id = od.index_file_id
        LEFT JOIN audit_logs al
            ON al.file_id = m.id
            AND al.interpretation_id = ii.interpretation_id
        WHERE ii.interpretation_id = ?1
        ORDER BY m.ingest_id DESC
        "#,
        interpretation_id,
        paths_json,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;

    let mut ids: HashMap<PathId, FileMeasurementRowId> = HashMap::with_capacity(rows.len());
    let mut files: HashMap<FileMeasurementRowId, FileDetails> = HashMap::with_capacity(rows.len());
    for row in rows {
        let path = PathId::new(row.data_source_uuid, &row.rel_path);
        if ids.contains_key(&path) {
            continue;
        }
        ids.insert(path, row.id);
        let media = row
            .media_measurements
            .as_deref()
            .map(json::decode)
            .transpose()?;
        let audit_log = row.audit_log.as_deref().map(json::decode).transpose()?;
        files.insert(
            row.id,
            FileDetails {
                data_source_uuid: row.data_source_uuid,
                data_source_name: row.data_source_name,
                rel_path: row.rel_path.into(),
                size: row.size,
                mtime: row.mtime,
                mime_type: row.mime_type,
                media,
                audit_log,
                movies: Vec::new(),
                episodes: Vec::new(),
                series_extras: Vec::new(),
                disc: DiscFacts::from_db(
                    row.disc_kind,
                    row.disc_root,
                    row.disc_member_count,
                    row.disc_size,
                    row.disc_duration_ms,
                )?,
                part_of_disc: row.part_of_disc.map(Into::into),
            },
        );
    }
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let ids_json = serde_json::to_string(&files.keys().map(|id| id.0).collect::<Vec<_>>())
        .expect("row IDs serialize to JSON");

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
        WHERE
            mf.file_id IN (SELECT value FROM json_each(?1))
            AND mo.interpretation_id = ?2
        ORDER BY mo.sort_title, mo.id
        "#,
        ids_json,
        interpretation_id,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?
    {
        if let Some(file) = files.get_mut(&r.file_id) {
            file.movies.push(FileMovie {
                uuid: r.uuid,
                title: r.title,
                release_year: r.release_year,
                role: r.role.as_deref().map(FileRole::from_db_str).transpose()?,
                manifestation: r.manifestation.map(|manifestation| manifestation as usize),
                part: r.part,
            });
        }
    }

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
        WHERE
            ef.file_id IN (SELECT value FROM json_each(?1))
            AND s.interpretation_id = ?2
        ORDER BY s.sort_name, s.id, se.number IS NULL, se.number, e.number
        "#,
        ids_json,
        interpretation_id,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?
    {
        if let Some(file) = files.get_mut(&r.file_id) {
            file.episodes.push(FileEpisode {
                series_uuid: r.series_uuid,
                series_name: r.series_name,
                season_number: r.season_number,
                episode_number: r.episode_number,
                title: r.title,
            });
        }
    }

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
        WHERE
            xf.file_id IN (SELECT value FROM json_each(?1))
            AND s.interpretation_id = ?2
        ORDER BY s.sort_name, s.id
        "#,
        ids_json,
        interpretation_id,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?
    {
        if let Some(file) = files.get_mut(&r.file_id) {
            file.series_extras.push(FileSeriesExtra {
                series_uuid: r.series_uuid,
                series_name: r.series_name,
                disc: r.disc,
            });
        }
    }

    Ok(paths
        .iter()
        .filter_map(|path| ids.get(path))
        .filter_map(|id| files.remove(id))
        .collect())
}

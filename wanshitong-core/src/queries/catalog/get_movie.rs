use crate::json;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{
        DataSourceUuid, FileRole, IngestUuid, InterpretationRowId, MovieDetails, MovieFile,
        MovieRowId, MovieUuid, SubjectAuditLog,
    },
};

pub async fn get_movie(
    ctx: &CoreContext,
    uuid: MovieUuid,
) -> Result<Option<MovieDetails>, CoreError> {
    let record = sqlx::query!(
        r#"
        SELECT
            -- Movie identity
            m.id AS "id!: MovieRowId",
            m.interpretation_id AS "interpretation_id!: InterpretationRowId",
            m.title,
            m.tmdb_id,
            m.imdb_id,
            m.tvdb_id,

            -- Movie metadata
            m.original_title,
            m.original_language,
            m.release_year,
            m.tagline,
            m.overview,
            m.runtime_seconds,
            m.tmdb_poster_path,

            -- Data source
            ds.uuid AS "data_source_uuid: DataSourceUuid",
            ds.name AS data_source_name,

            -- File
            fm.rel_path,

            -- Ingest
            i.uuid AS "ingest_uuid: IngestUuid",

            -- Media measurements
            fmm.data AS "media_data?"

        FROM movies m

        -- Chosen file for this movie
        JOIN movie_files mf
            ON mf.movie_id = m.id

        JOIN file_measurements fm
            ON fm.id = mf.file_id

        -- Media measurements
        LEFT JOIN file_media_measurements fmm
            ON fmm.id = fm.media_measurement_id

        -- Ingest + data source provenance
        JOIN ingests i
            ON i.id = fm.ingest_id

        JOIN data_sources ds
            ON ds.id = fm.data_source_id

        WHERE m.uuid = ?1

        -- A movie can have several main files, or none. Show a main file if
        -- there is one, then a file of unknown role, then an extra.
        ORDER BY
            CASE WHEN mf.role = 'main' THEN 0 WHEN mf.role IS NULL THEN 1 ELSE 2 END,
            fm.rel_path
        LIMIT 1;
        "#,
        uuid
    )
    .fetch_optional(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;

    let Some(r) = record else {
        return Ok(None);
    };

    let media = r.media_data.as_deref().map(json::decode).transpose()?;

    // Fetch all files associated with this movie
    let files = fetch_movie_files(ctx, r.id, r.interpretation_id).await?;

    Ok(Some(MovieDetails {
        id: r.id,
        uuid,
        title: r.title,
        tmdb_id: r.tmdb_id,
        imdb_id: r.imdb_id,
        tvdb_id: r.tvdb_id,
        original_title: r.original_title,
        original_language: r.original_language,
        release_year: r.release_year,
        tagline: r.tagline,
        overview: r.overview,
        runtime_seconds: r.runtime_seconds,
        tmdb_poster_path: r.tmdb_poster_path,
        data_source_uuid: r.data_source_uuid,
        data_source_name: r.data_source_name,
        rel_path: r.rel_path.into(),
        ingest_uuid: r.ingest_uuid,
        media,
        files,
    }))
}

async fn fetch_movie_files(
    ctx: &CoreContext,
    movie_id: MovieRowId,
    interpretation_id: InterpretationRowId,
) -> Result<Vec<MovieFile>, CoreError> {
    let records = sqlx::query!(
        r#"
        SELECT
            ds.uuid AS "data_source_uuid!: DataSourceUuid",
            fm.rel_path,
            mf.role,
            al.data AS "audit_log?"
        FROM movie_files mf
        JOIN file_measurements fm
            ON fm.id = mf.file_id
        JOIN data_sources ds
            ON ds.id = fm.data_source_id
        LEFT JOIN audit_logs al
            ON al.file_id = mf.file_id
            AND al.interpretation_id = ?2
        WHERE mf.movie_id = ?1
        "#,
        movie_id,
        interpretation_id,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;

    let files = records
        .into_iter()
        .map(|r| {
            let audit_log = match r.audit_log {
                Some(blob) => json::decode(&blob)?,
                None => SubjectAuditLog::default(),
            };
            Ok(MovieFile {
                data_source_uuid: r.data_source_uuid,
                rel_path: r.rel_path.into(),
                role: r.role.as_deref().map(FileRole::from_db_str).transpose()?,
                audit_log,
            })
        })
        .collect::<Result<Vec<_>, CoreError>>()?;
    Ok(files)
}

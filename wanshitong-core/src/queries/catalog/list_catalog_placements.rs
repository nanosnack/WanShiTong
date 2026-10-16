use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{
        CatalogFile, CatalogPlacements, CatalogWork, CatalogWorkKind, DataSourceUuid, FilePart,
        FileRole, InterpretationRowId, PlacedFile,
    },
};
use std::collections::HashMap;
use uuid::Uuid;

/// Where an interpretation's catalog placed its files, and the works it
/// placed them in.
pub async fn list_catalog_placements(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
) -> Result<CatalogPlacements, CoreError> {
    let mut txn = ctx.reader.begin().await.map_err(CoreErrorContext::from)?;
    let mut placements = CatalogPlacements::default();

    let mut movies = HashMap::new();
    for row in sqlx::query!(
        r#"
        SELECT
            id AS "id!",
            uuid AS "uuid!: Uuid",
            title,
            release_year,
            tmdb_id,
            imdb_id,
            tvdb_id
        FROM movies
        WHERE interpretation_id = ?
        "#,
        interpretation_id,
    )
    .fetch_all(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?
    {
        movies.insert(row.id, placements.works.len());
        placements.works.push(CatalogWork {
            kind: CatalogWorkKind::Movie,
            uuid: row.uuid,
            title: row.title,
            year: row.release_year,
            tmdb_id: row.tmdb_id,
            imdb_id: row.imdb_id,
            tvdb_id: row.tvdb_id,
        });
    }
    let mut series = HashMap::new();
    for row in sqlx::query!(
        r#"
        SELECT
            id AS "id!",
            uuid AS "uuid!: Uuid",
            name,
            first_air_year,
            tmdb_id,
            imdb_id,
            tvdb_id
        FROM tv_series
        WHERE interpretation_id = ?
        "#,
        interpretation_id,
    )
    .fetch_all(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?
    {
        series.insert(row.id, placements.works.len());
        placements.works.push(CatalogWork {
            kind: CatalogWorkKind::TvSeries,
            uuid: row.uuid,
            title: row.name,
            year: row.first_air_year,
            tmdb_id: row.tmdb_id,
            imdb_id: row.imdb_id,
            tvdb_id: row.tvdb_id,
        });
    }

    for row in sqlx::query!(
        r#"
        SELECT
            mf.movie_id AS "work_id!",
            mf.role,
            mf.manifestation AS "manifestation: u32",
            mf.part AS "part: u32",
            ds.uuid AS "data_source_uuid!: DataSourceUuid",
            ds.name AS data_source_name,
            fm.rel_path
        FROM movie_files mf
        JOIN movies m
            ON m.id = mf.movie_id
        JOIN file_measurements fm
            ON fm.id = mf.file_id
        JOIN data_sources ds
            ON ds.id = fm.data_source_id
        WHERE m.interpretation_id = ?
        "#,
        interpretation_id,
    )
    .fetch_all(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?
    {
        placements.files.push(PlacedFile {
            data_source_uuid: row.data_source_uuid,
            data_source_name: row.data_source_name,
            rel_path: row.rel_path.into(),
            work: movies[&row.work_id],
            part: FilePart::Movie {
                role: row.role.as_deref().map(FileRole::from_db_str).transpose()?,
                manifestation: row
                    .manifestation
                    .map(|manifestation| manifestation as usize),
                part: row.part,
            },
        });
    }
    for row in sqlx::query!(
        r#"
        SELECT
            s.id AS "work_id!",
            se.number AS "season: u32",
            e.number AS "episode!: u32",
            ds.uuid AS "data_source_uuid!: DataSourceUuid",
            ds.name AS data_source_name,
            fm.rel_path
        FROM tv_episode_files ef
        JOIN tv_episodes e
            ON e.id = ef.episode_id
        JOIN tv_seasons se
            ON se.id = e.season_id
        JOIN tv_series s
            ON s.id = se.series_id
        JOIN file_measurements fm
            ON fm.id = ef.file_id
        JOIN data_sources ds
            ON ds.id = fm.data_source_id
        WHERE s.interpretation_id = ?
        "#,
        interpretation_id,
    )
    .fetch_all(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?
    {
        placements.files.push(PlacedFile {
            data_source_uuid: row.data_source_uuid,
            data_source_name: row.data_source_name,
            rel_path: row.rel_path.into(),
            work: series[&row.work_id],
            part: FilePart::Episode {
                season: row.season,
                episode: row.episode,
            },
        });
    }
    for row in sqlx::query!(
        r#"
        SELECT
            xf.series_id AS "work_id!",
            ds.uuid AS "data_source_uuid!: DataSourceUuid",
            ds.name AS data_source_name,
            fm.rel_path,
            xf.disc AS "disc!: bool"
        FROM tv_series_extra_files xf
        JOIN tv_series s
            ON s.id = xf.series_id
        JOIN file_measurements fm
            ON fm.id = xf.file_id
        JOIN data_sources ds
            ON ds.id = fm.data_source_id
        WHERE s.interpretation_id = ?
        "#,
        interpretation_id,
    )
    .fetch_all(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?
    {
        placements.files.push(PlacedFile {
            data_source_uuid: row.data_source_uuid,
            data_source_name: row.data_source_name,
            rel_path: row.rel_path.into(),
            work: series[&row.work_id],
            part: if row.disc {
                FilePart::SeriesDisc
            } else {
                FilePart::SeriesExtra
            },
        });
    }
    for row in sqlx::query!(
        r#"
        SELECT
            ds.uuid AS "data_source_uuid!: DataSourceUuid",
            ds.name AS data_source_name,
            fm.rel_path
        FROM unidentified_videos uv
        JOIN file_measurements fm
            ON fm.id = uv.file_id
        JOIN data_sources ds
            ON ds.id = fm.data_source_id
        WHERE uv.interpretation_id = ?
        "#,
        interpretation_id,
    )
    .fetch_all(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?
    {
        placements.unidentified.push(CatalogFile {
            data_source_uuid: row.data_source_uuid,
            data_source_name: row.data_source_name,
            rel_path: row.rel_path.into(),
        });
    }
    txn.commit().await.map_err(CoreErrorContext::from)?;
    Ok(placements)
}

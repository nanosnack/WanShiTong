use crate::json;
use crate::sort_key::sort_key;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{
        FileMeasurementRowId, FileRole, InterpretationRowId, MovieKbData, MovieRowId, MovieUuid,
        SubjectAuditLog, TvEpisodeRowId, TvSeasonRowId, TvSeriesKbData, TvSeriesRowId,
        TvSeriesUuid,
    },
};
use sqlx::SqliteTransaction;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// A fully resolved catalog for an interpretation, ready to be persisted.
#[derive(Default)]
pub struct NewCatalog {
    pub movies: Vec<NewMovie>,
    pub tv_series: Vec<NewTvSeries>,
    pub audit_logs: Vec<(FileMeasurementRowId, SubjectAuditLog)>,
    /// The video files that no identified work includes, and that weren't
    /// excluded.
    pub unidentified_videos: Vec<FileMeasurementRowId>,
    /// Every video file, identified or not: those with moving pictures.
    pub videos: Vec<FileMeasurementRowId>,
}

#[derive(Debug, Default)]
pub struct NewMovie {
    pub title: String,
    pub tmdb_id: Option<u32>,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<u32>,
    pub release_year: Option<u32>,
    pub kb_data: MovieKbData,
    /// The cuts its manifestations realize. See *Expression* in
    /// `spec/domain-model.md`.
    pub expressions: Vec<NewMovieExpression>,
    pub files: Vec<NewMovieFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewMovieExpression {
    /// E.g. `Director's Cut`, or None for the unspecified expression.
    pub edition: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct NewMovieFile {
    pub file_id: FileMeasurementRowId,
    /// None when nothing says what the file's role is.
    pub role: Option<FileRole>,
    /// For a main file, the expression its manifestation realizes, by its position in
    /// the movie's expressions. None for any other file.
    pub expression: Option<usize>,
    /// For a main file, which of the movie's manifestations it's in, numbered from 0
    /// in the order of the manifestations' first files (see *Manifestation* in
    /// `spec/domain-model.md`). None for any other file.
    pub manifestation: Option<usize>,
    /// For a main file, which part of its manifestation it is, if its name numbers it
    /// as one, e.g. 1 for `CD1`. None otherwise.
    pub part: Option<u32>,
}

#[derive(Debug, Default)]
pub struct NewTvSeries {
    pub name: String,
    pub original_name: Option<String>,
    pub tmdb_id: Option<u32>,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<u32>,
    pub kb_data: TvSeriesKbData,
    pub episodes: Vec<NewEpisode>,
    /// Files that belong to the series without being episodes of it.
    ///
    /// TODO: extras are works, like episodes, so this should be a list of
    /// extras with their files once something identifies extras. See
    /// `tv_series_extra_files` in the schema.
    pub extras: Vec<FileMeasurementRowId>,
    /// Discs kept with the series, by their index files: neither episodes
    /// nor extras of it. See *Disc* in `spec/domain-model.md`.
    pub discs: Vec<FileMeasurementRowId>,
}

/// A file of one or more consecutive episodes of a season.
#[derive(Debug)]
pub struct NewEpisode {
    pub file_id: FileMeasurementRowId,
    /// None when nothing says the season, and the episodes are numbered
    /// through the whole series.
    pub season_number: Option<u32>,
    /// The first and last episode in the file.
    pub episode_range: [u32; 2],
    pub episode_title: Option<String>,
}

/// Persist a resolved catalog for an interpretation in a single transaction.
pub async fn record_catalog(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
    catalog: NewCatalog,
) -> Result<(), CoreError> {
    let mut txn = ctx.writer.begin().await.map_err(CoreErrorContext::from)?;
    validate_catalog(&mut txn, &catalog).await?;
    let NewCatalog {
        movies,
        tv_series,
        audit_logs,
        unidentified_videos,
        videos,
    } = catalog;

    ensure_no_catalog(&mut txn, interpretation_id).await?;
    let fact_file_ids = fact_file_ids(&mut txn, interpretation_id).await?;
    let foreign_file = movies
        .iter()
        .flat_map(|movie| movie.files.iter().map(|file| file.file_id))
        .chain(tv_series.iter().flat_map(NewTvSeries::file_ids))
        .chain(audit_logs.iter().map(|(file_id, _)| *file_id))
        .chain(unidentified_videos.iter().copied())
        .chain(videos.iter().copied())
        .find(|file_id| !fact_file_ids.contains(file_id));
    if let Some(file_id) = foreign_file {
        let file = file_paths(&mut txn, &[file_id]).await?.remove(0);
        return Err(CatalogError::FileNotInInterpretation(file).into());
    }

    for movie in movies {
        insert_movie(&mut txn, interpretation_id, movie).await?;
    }
    for series in tv_series {
        insert_tv_series(&mut txn, interpretation_id, series).await?;
    }
    persist_audit_logs(&mut txn, interpretation_id, audit_logs).await?;
    for file_id in unidentified_videos {
        sqlx::query!(
            "INSERT INTO unidentified_videos (interpretation_id, file_id) VALUES (?, ?)",
            interpretation_id,
            file_id,
        )
        .execute(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?;
    }
    for file_id in videos {
        sqlx::query!(
            "INSERT INTO video_files (interpretation_id, file_id) VALUES (?, ?)",
            interpretation_id,
            file_id,
        )
        .execute(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?;
    }
    txn.commit().await.map_err(CoreErrorContext::from)?;

    Ok(())
}

/// Files are named by data source and path, so that an error says which files
/// to look at.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("interpretation {0} already has a catalog")]
    AlreadyRecorded(InterpretationRowId),
    #[error("file {0} is not in the interpretation's ingest set")]
    FileNotInInterpretation(String),
    #[error("invalid episode range {0:?}")]
    InvalidEpisodeRange([u32; 2]),
    #[error("movie has no files: {0}")]
    MovieWithoutFiles(String),
    #[error("movie {title:?} has a sample file, which does not belong in a catalog: {file}")]
    SampleFile { title: String, file: String },
    #[error(
        "movies {first:?} and {second:?} share {id}, so they should be one movie. \
         Their files: {}",
        files.join(", ")
    )]
    SharedMovieId {
        id: String,
        first: String,
        second: String,
        files: Vec<String>,
    },
    #[error(
        "movie {title:?}'s expressions are wrong: {problem}. Each main file realizes one, \
         no other file does, and each is of a different edition and realized"
    )]
    InvalidMovieExpressions { title: String, problem: String },
    #[error(
        "movie {title:?}'s manifestations are wrong: {problem}. Each main file is in one, no other \
         file is, a manifestation's files realize one expression, and a part is a main file's"
    )]
    InvalidMovieManifestations { title: String, problem: String },
    #[error("TV series has no episodes or extras: {0}")]
    TvSeriesWithoutFiles(String),
    #[error(
        "TV series {first:?} and {second:?} share {id}, so they should be one series. \
         Their files: {}",
        files.join(", ")
    )]
    SharedTvSeriesId {
        id: String,
        first: String,
        second: String,
        files: Vec<String>,
    },
}

/// Checks that only need the catalog itself, and the files' paths to report
/// what's wrong.
async fn validate_catalog(
    txn: &mut SqliteTransaction<'_>,
    catalog: &NewCatalog,
) -> Result<(), CoreError> {
    let mut movies_by_id = HashMap::<String, &NewMovie>::new();
    for movie in &catalog.movies {
        let ids = [
            movie.tmdb_id.map(|id| format!("TMDB ID {id}")),
            movie.imdb_id.as_ref().map(|id| format!("IMDB ID {id}")),
            movie.tvdb_id.map(|id| format!("TVDB ID {id}")),
        ];
        for id in ids.into_iter().flatten() {
            if let Some(first) = movies_by_id.insert(id.clone(), movie) {
                let file_ids: Vec<_> = first
                    .files
                    .iter()
                    .chain(&movie.files)
                    .map(|f| f.file_id)
                    .collect();
                return Err(CatalogError::SharedMovieId {
                    id,
                    first: first.title.clone(),
                    second: movie.title.clone(),
                    files: file_paths(txn, &file_ids).await?,
                }
                .into());
            }
        }
        if movie.files.is_empty() {
            return Err(CatalogError::MovieWithoutFiles(movie.title.clone()).into());
        }
        if let Some(sample) = movie
            .files
            .iter()
            .find(|file| file.role == Some(FileRole::Sample))
        {
            return Err(CatalogError::SampleFile {
                title: movie.title.clone(),
                file: file_paths(txn, &[sample.file_id]).await?.remove(0),
            }
            .into());
        }
        if let Some(problem) = movie_expressions_problem(movie) {
            return Err(CatalogError::InvalidMovieExpressions {
                title: movie.title.clone(),
                problem,
            }
            .into());
        }
        if let Some(problem) = movie_manifestations_problem(movie) {
            return Err(CatalogError::InvalidMovieManifestations {
                title: movie.title.clone(),
                problem,
            }
            .into());
        }
    }
    let mut series_by_id = HashMap::<String, &NewTvSeries>::new();
    for series in &catalog.tv_series {
        let ids = [
            series.tmdb_id.map(|id| format!("TMDB ID {id}")),
            series.imdb_id.as_ref().map(|id| format!("IMDB ID {id}")),
            series.tvdb_id.map(|id| format!("TVDB ID {id}")),
        ];
        for id in ids.into_iter().flatten() {
            if let Some(first) = series_by_id.insert(id.clone(), series) {
                let file_ids: Vec<_> = first.file_ids().chain(series.file_ids()).collect();
                return Err(CatalogError::SharedTvSeriesId {
                    id,
                    first: first.name.clone(),
                    second: series.name.clone(),
                    files: file_paths(txn, &file_ids).await?,
                }
                .into());
            }
        }
        if series.episodes.is_empty() && series.extras.is_empty() {
            return Err(CatalogError::TvSeriesWithoutFiles(series.name.clone()).into());
        }
        for episode in &series.episodes {
            let [first, last] = episode.episode_range;
            if first > last {
                return Err(CatalogError::InvalidEpisodeRange(episode.episode_range).into());
            }
        }
    }
    Ok(())
}

/// What's wrong with a movie's expressions, if anything: each main file
/// realizes one of them, no other file does, and each is of a different
/// edition and realized by some file.
fn movie_expressions_problem(movie: &NewMovie) -> Option<String> {
    let editions: HashSet<Option<&str>> = movie
        .expressions
        .iter()
        .map(|expression| expression.edition.as_deref())
        .collect();
    if editions.len() < movie.expressions.len() {
        return Some("two are of one edition".into());
    }
    if movie
        .expressions
        .iter()
        .any(|expression| expression.edition.as_deref() == Some(""))
    {
        return Some("an edition is empty".into());
    }
    let mut realized = HashSet::new();
    for file in &movie.files {
        match (file.role, file.expression) {
            (Some(FileRole::Main), None) => {
                return Some(format!("main file {} realizes none", file.file_id.0));
            }
            (Some(FileRole::Main), Some(i)) if i >= movie.expressions.len() => {
                return Some(format!(
                    "main file {} realizes one it hasn't",
                    file.file_id.0
                ));
            }
            (Some(FileRole::Main), Some(i)) => {
                realized.insert(i);
            }
            (_, Some(_)) => {
                return Some(format!(
                    "file {}, not a main file, realizes one",
                    file.file_id.0
                ));
            }
            (_, None) => (),
        }
    }
    (realized.len() < movie.expressions.len()).then(|| "one isn't realized".into())
}

/// Why a movie's manifestations are wrong, if they are: each main file is in one, no
/// other file is, the files of one manifestation realize one expression, and only a
/// main file is a part.
fn movie_manifestations_problem(movie: &NewMovie) -> Option<String> {
    let mut expression_of_copy = HashMap::<usize, Option<usize>>::new();
    for file in &movie.files {
        let is_main = file.role == Some(FileRole::Main);
        match (is_main, file.manifestation) {
            (true, None) => return Some(format!("main file {} is in none", file.file_id.0)),
            (false, Some(_)) => {
                return Some(format!(
                    "file {}, not a main file, is in one",
                    file.file_id.0
                ));
            }
            (true, Some(manifestation)) => {
                if *expression_of_copy
                    .entry(manifestation)
                    .or_insert(file.expression)
                    != file.expression
                {
                    return Some(format!(
                        "manifestation {manifestation}'s files realize different expressions"
                    ));
                }
            }
            (false, None) => (),
        }
        if !is_main && file.part.is_some() {
            return Some(format!(
                "file {}, not a main file, is a part",
                file.file_id.0
            ));
        }
    }
    None
}

/// Each file as `data source/path`, or its row ID if it doesn't exist.
async fn file_paths(
    txn: &mut SqliteTransaction<'_>,
    file_ids: &[FileMeasurementRowId],
) -> Result<Vec<String>, CoreError> {
    let mut paths = Vec::new();
    for &file_id in file_ids {
        let path = sqlx::query!(
            r#"
            SELECT ds.name, fm.rel_path
            FROM file_measurements fm
            JOIN data_sources ds
                ON ds.id = fm.data_source_id
            WHERE fm.id = ?
            "#,
            file_id,
        )
        .fetch_optional(&mut **txn)
        .await
        .map_err(CoreErrorContext::from)?;
        paths.push(match path {
            Some(r) => format!("{}/{}", r.name, r.rel_path),
            None => format!("{file_id:?}"),
        });
    }
    Ok(paths)
}

/// Interpretations are immutable, so a catalog may only be recorded once.
async fn ensure_no_catalog(
    txn: &mut SqliteTransaction<'_>,
    interpretation_id: InterpretationRowId,
) -> Result<(), CoreError> {
    let exists = sqlx::query_scalar!(
        r#"
        SELECT
            EXISTS (SELECT 1 FROM movies WHERE interpretation_id = ?1)
            OR EXISTS (SELECT 1 FROM tv_series WHERE interpretation_id = ?1)
            OR EXISTS (SELECT 1 FROM audit_logs WHERE interpretation_id = ?1)
            AS "exists!: bool"
        "#,
        interpretation_id,
    )
    .fetch_one(&mut **txn)
    .await
    .map_err(CoreErrorContext::from)?;
    if exists {
        return Err(CatalogError::AlreadyRecorded(interpretation_id).into());
    }
    Ok(())
}

async fn fact_file_ids(
    txn: &mut SqliteTransaction<'_>,
    interpretation_id: InterpretationRowId,
) -> Result<HashSet<FileMeasurementRowId>, CoreError> {
    let ids = sqlx::query_scalar!(
        r#"
        SELECT fm.id AS "id!: FileMeasurementRowId"
        FROM file_measurements fm
        JOIN interpretation_ingests ii
            ON ii.ingest_id = fm.ingest_id
        WHERE ii.interpretation_id = ?
        "#,
        interpretation_id,
    )
    .fetch_all(&mut **txn)
    .await
    .map_err(CoreErrorContext::from)?;
    Ok(ids.into_iter().collect())
}

async fn persist_audit_logs(
    txn: &mut SqliteTransaction<'_>,
    interpretation_id: InterpretationRowId,
    audit_logs: Vec<(FileMeasurementRowId, SubjectAuditLog)>,
) -> Result<(), CoreError> {
    for (file_id, log) in audit_logs {
        let data = json::encode(&log);
        sqlx::query!(
            r#"
            INSERT INTO audit_logs (interpretation_id, file_id, data)
            VALUES (?, ?, ?)
            ON CONFLICT (interpretation_id, file_id) DO UPDATE SET data = EXCLUDED.data
            "#,
            interpretation_id,
            file_id,
            data,
        )
        .execute(&mut **txn)
        .await
        .map_err(CoreErrorContext::from)?;
    }
    Ok(())
}

async fn insert_movie(
    txn: &mut SqliteTransaction<'_>,
    interpretation_id: InterpretationRowId,
    movie: NewMovie,
) -> Result<(), CoreError> {
    let NewMovie {
        expressions,
        files,
        title,
        tmdb_id,
        imdb_id,
        tvdb_id,
        release_year,
        kb_data:
            MovieKbData {
                original_title,
                original_language,
                tagline,
                overview,
                release_date,
                poster_path,
                runtime,
            },
    } = movie;
    let year = release_date
        .map(|d| i64::from(d.year))
        .or(release_year.map(i64::from));
    let month = release_date.map(|d| i64::from(d.month));
    let day = release_date.map(|d| i64::from(d.day));
    let runtime_seconds = runtime.map(|d| d.as_secs() as i64);
    let sort_title = sort_key(&title);

    let movie_uuid = MovieUuid(Uuid::new_v4());
    let movie_id = sqlx::query_scalar!(
        r#"
        INSERT INTO movies (
            uuid,
            interpretation_id,
            title,
            tmdb_id,
            imdb_id,
            tvdb_id,

            original_title,
            original_language,
            tagline,
            overview,
            release_year,
            release_month,
            release_day,
            runtime_seconds,
            tmdb_poster_path,
            sort_title
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        RETURNING id AS "id!: MovieRowId"
        "#,
        movie_uuid,
        interpretation_id,
        title,
        tmdb_id,
        imdb_id,
        tvdb_id,
        original_title,
        original_language,
        tagline,
        overview,
        year,
        month,
        day,
        runtime_seconds,
        poster_path,
        sort_title,
    )
    .fetch_one(&mut **txn)
    .await
    .map_err(CoreErrorContext::from)?;

    let mut expression_ids = Vec::with_capacity(expressions.len());
    for NewMovieExpression { edition } in expressions {
        let expression_id = sqlx::query_scalar!(
            r#"
            INSERT INTO movie_expressions (movie_id, edition)
            VALUES (?, ?)
            RETURNING id
            "#,
            movie_id,
            edition,
        )
        .fetch_one(&mut **txn)
        .await
        .map_err(CoreErrorContext::from)?;
        expression_ids.push(expression_id);
    }

    // Insert all files associated with this movie
    for NewMovieFile {
        file_id,
        role,
        expression,
        manifestation,
        part,
    } in files
    {
        let role_str = role.map(FileRole::as_db_str);
        let expression_id = expression.map(|i| expression_ids[i]);
        let manifestation = manifestation.map(|manifestation| manifestation as i64);
        sqlx::query!(
            r#"
            INSERT INTO movie_files (movie_id, file_id, role, expression_id, manifestation, part)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            movie_id,
            file_id,
            role_str,
            expression_id,
            manifestation,
            part,
        )
        .execute(&mut **txn)
        .await
        .map_err(CoreErrorContext::from)?;
    }
    Ok(())
}

async fn insert_tv_series(
    txn: &mut SqliteTransaction<'_>,
    interpretation_id: InterpretationRowId,
    series: NewTvSeries,
) -> Result<(), CoreError> {
    let NewTvSeries {
        name,
        original_name,
        tmdb_id,
        imdb_id,
        tvdb_id,
        kb_data:
            TvSeriesKbData {
                original_name: kb_original_name,
                original_language,
                tagline,
                overview,
                first_air_date,
                poster_path,
            },
        episodes,
        extras,
        discs,
    } = series;
    let original_name = original_name.or(kb_original_name);
    let first_air_year = first_air_date.map(|d| i64::from(d.year));
    let first_air_month = first_air_date.map(|d| i64::from(d.month));
    let first_air_day = first_air_date.map(|d| i64::from(d.day));
    let sort_name = sort_key(&name);
    let series_uuid = TvSeriesUuid(Uuid::new_v4());
    let series_id = sqlx::query_scalar!(
        r#"
        INSERT INTO tv_series (
            uuid,
            interpretation_id,
            name,
            original_name,
            tmdb_id,
            imdb_id,
            tvdb_id,

            original_language,
            tagline,
            overview,
            first_air_year,
            first_air_month,
            first_air_day,
            tmdb_poster_path,
            sort_name
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        RETURNING id AS "id!: TvSeriesRowId"
        "#,
        series_uuid,
        interpretation_id,
        name,
        original_name,
        tmdb_id,
        imdb_id,
        tvdb_id,
        original_language,
        tagline,
        overview,
        first_air_year,
        first_air_month,
        first_air_day,
        poster_path,
        sort_name,
    )
    .fetch_one(&mut **txn)
    .await
    .map_err(CoreErrorContext::from)?;

    for NewEpisode {
        file_id,
        season_number,
        episode_range: [first, last],
        episode_title,
    } in episodes
    {
        // The no-op update makes RETURNING give the existing row's ID.
        let season_id = sqlx::query_scalar!(
            r#"
            INSERT INTO tv_seasons (series_id, number)
            VALUES (?, ?)
            ON CONFLICT DO UPDATE SET number = EXCLUDED.number
            RETURNING id AS "id!: TvSeasonRowId"
            "#,
            series_id,
            season_number,
        )
        .fetch_one(&mut **txn)
        .await
        .map_err(CoreErrorContext::from)?;
        for episode_number in first..=last {
            // An episode in several files keeps the first file's title.
            let episode_id = sqlx::query_scalar!(
                r#"
                INSERT INTO tv_episodes (season_id, number, title)
                VALUES (?, ?, ?)
                ON CONFLICT DO UPDATE SET title = coalesce(title, EXCLUDED.title)
                RETURNING id AS "id!: TvEpisodeRowId"
                "#,
                season_id,
                episode_number,
                episode_title,
            )
            .fetch_one(&mut **txn)
            .await
            .map_err(CoreErrorContext::from)?;
            sqlx::query!(
                r#"
                INSERT INTO tv_episode_files (episode_id, file_id)
                VALUES (?, ?)
                ON CONFLICT DO NOTHING
                "#,
                episode_id,
                file_id,
            )
            .execute(&mut **txn)
            .await
            .map_err(CoreErrorContext::from)?;
        }
    }
    for file_id in extras {
        sqlx::query!(
            r#"
            INSERT INTO tv_series_extra_files (series_id, file_id)
            VALUES (?, ?)
            ON CONFLICT DO NOTHING
            "#,
            series_id,
            file_id,
        )
        .execute(&mut **txn)
        .await
        .map_err(CoreErrorContext::from)?;
    }
    for file_id in discs {
        sqlx::query!(
            r#"
            INSERT INTO tv_series_extra_files (series_id, file_id, disc)
            VALUES (?, ?, 1)
            ON CONFLICT DO NOTHING
            "#,
            series_id,
            file_id,
        )
        .execute(&mut **txn)
        .await
        .map_err(CoreErrorContext::from)?;
    }
    Ok(())
}

impl NewMovie {
    /// Whether it has an external ID, which says which movie it is.
    pub fn is_identified(&self) -> bool {
        self.tmdb_id.is_some() || self.imdb_id.is_some() || self.tvdb_id.is_some()
    }
}

impl NewTvSeries {
    /// Whether it has an external ID, which says which series it is.
    pub fn is_identified(&self) -> bool {
        self.tmdb_id.is_some() || self.imdb_id.is_some() || self.tvdb_id.is_some()
    }

    /// The files of the series' episodes, extras and discs.
    pub fn file_ids(&self) -> impl Iterator<Item = FileMeasurementRowId> + '_ {
        self.episodes
            .iter()
            .map(|episode| episode.file_id)
            .chain(self.extras.iter().copied())
            .chain(self.discs.iter().copied())
    }
}

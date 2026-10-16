use super::{ApiError, interpretation};
use crate::WebContext;
use axum::{
    Json,
    extract::{
        Path, Query, State,
        rejection::{PathRejection, QueryRejection},
    },
};
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;
use wanshitong_api::v1::{self, ErrorCode};
use wanshitong_app::domain::{
    AssertedFile, Assertion, ClaimPredicate, ClaimSource, DirectoryFile, ExternalId, FileDetails,
    FileEpisode, FileMediaMeasurements, FileMovie, FileRole, FileSeriesExtra, KbLookupResult,
    KbQueryEvent, KbSearchResult, MovieDetails, Rank, RankedClaimPredicate, SubjectAuditLog,
    TvSeriesDetails, WorkKind,
};
use wanshitong_app::domain::{DataSourceUuid, DiscKind, InterpretationUuid};
use wanshitong_app::queries::{self, Directory, FileEvidence, TopDirectory, Work, WorkEvidence};

/// Get a work's evidence
///
/// Everything WST knows about how it identified the work.
#[utoipa::path(
    get,
    path = "/works/{uuid}",
    tag = "Evidence",
    params(
        ("uuid" = Uuid, Path, description = "The work's, valid only in its interpretation."),
        v1::WorkQuery,
    ),
    responses((status = OK, body = v1::WorkEvidence), ApiError),
)]
pub(super) async fn get_work(
    State(ctx): State<WebContext>,
    uuid: Result<Path<Uuid>, PathRejection>,
    query: Result<Query<v1::WorkQuery>, QueryRejection>,
) -> Result<Json<v1::WorkEvidence>, ApiError> {
    let Path(uuid) = uuid?;
    let Query(v1::WorkQuery { directory_files }) = query?;
    let WorkEvidence {
        interpretation: details,
        work,
        files,
        top_directories,
        assertions,
    } = queries::get_work_evidence(&ctx.app, uuid)
        .await?
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, format!("no work {uuid}")))?;
    Ok(Json(v1::WorkEvidence {
        interpretation: interpretation(&details),
        work: match work {
            Work::Movie(movie) => movie_work(*movie),
            Work::TvSeries(series) => series_work(*series),
        },
        files: files.into_iter().map(file).collect(),
        top_directories: top_directories
            .into_iter()
            .map(|r| top_directory(r, directory_files))
            .collect(),
        assertions: assertions.into_iter().map(assertion).collect(),
    }))
}

/// Get a file's evidence
///
/// Everything WST knows about a file, e.g. an unidentified video, to identify
/// it by.
#[utoipa::path(
    get,
    path = "/files/evidence",
    tag = "Evidence",
    params(v1::FileQuery),
    responses((status = OK, body = v1::FileEvidence), ApiError),
)]
pub(super) async fn get_file(
    State(ctx): State<WebContext>,
    query: Result<Query<v1::FileQuery>, QueryRejection>,
) -> Result<Json<v1::FileEvidence>, ApiError> {
    let Query(v1::FileQuery {
        interpretation: requested,
        data_source,
        path,
        directory_files,
    }) = query?;
    let rel_path = PathBuf::from(&path);
    let FileEvidence {
        interpretation: details,
        file: details_of_file,
        top_directory: file_top_directory,
        assertions,
    } = queries::get_file_evidence(
        &ctx.app,
        requested.map(InterpretationUuid),
        DataSourceUuid(data_source),
        &rel_path,
    )
    .await?
    .ok_or_else(|| {
        ApiError::new(
            ErrorCode::NotFound,
            format!("the interpretation saw no file {path} in data source {data_source}"),
        )
    })?;
    Ok(Json(v1::FileEvidence {
        interpretation: interpretation(&details),
        file: file(details_of_file),
        top_directory: file_top_directory.map(|r| top_directory(r, directory_files)),
        assertions: assertions.into_iter().map(assertion).collect(),
    }))
}

fn movie_work(movie: MovieDetails) -> v1::Work {
    let MovieDetails {
        uuid,
        title,
        tmdb_id,
        imdb_id,
        tvdb_id,
        release_year,
        runtime_seconds,
        ..
    } = movie;
    v1::Work::Movie {
        uuid: uuid.0,
        title,
        year: release_year,
        external_ids: external_ids(tmdb_id, imdb_id, tvdb_id),
        runtime_seconds,
    }
}

fn series_work(series: TvSeriesDetails) -> v1::Work {
    let TvSeriesDetails {
        uuid,
        name,
        tmdb_id,
        imdb_id,
        tvdb_id,
        first_air_year,
        ..
    } = series;
    v1::Work::TvSeries {
        uuid: uuid.0,
        title: name,
        year: first_air_year,
        external_ids: external_ids(tmdb_id, imdb_id, tvdb_id),
    }
}

fn external_ids(
    tmdb_id: Option<i64>,
    imdb_id: Option<String>,
    tvdb_id: Option<i64>,
) -> Vec<v1::ExternalId> {
    let tmdb = tmdb_id.map(|id| v1::ExternalId::Tmdb(id as u32));
    let imdb = imdb_id.map(v1::ExternalId::Imdb);
    let tvdb = tvdb_id.map(|id| v1::ExternalId::Tvdb(id as u32));
    [tmdb, imdb, tvdb].into_iter().flatten().collect()
}

fn file(file: FileDetails) -> v1::File {
    let FileDetails {
        data_source_uuid,
        data_source_name,
        rel_path,
        size,
        mtime: _,
        mime_type: _,
        media,
        audit_log,
        movies,
        episodes,
        series_extras,
        disc,
        part_of_disc,
    } = file;
    let placements = movies
        .into_iter()
        .map(movie_placement)
        .chain(episodes.into_iter().map(episode_placement))
        .chain(series_extras.into_iter().map(series_extra_placement))
        .collect();
    let SubjectAuditLog { claims, kb_queries } = audit_log.unwrap_or_default();
    v1::File {
        data_source: v1::DataSource {
            uuid: data_source_uuid.0,
            name: data_source_name,
        },
        path: rel_path.to_string_lossy().into_owned(),
        size_bytes: size,
        placements,
        media: media.map(media_summary),
        claims: claims.into_iter().map(ranked_claim).collect(),
        kb_queries: kb_queries.into_iter().map(kb_query).collect(),
        disc: disc.map(|disc| v1_disc(disc.kind, disc.file_count, disc.duration)),
        part_of_disc: part_of_disc.map(|path| path.to_string_lossy().into_owned()),
    }
}

/// A disc as the API describes it.
pub(super) fn v1_disc(kind: DiscKind, file_count: u64, duration: Option<Duration>) -> v1::Disc {
    v1::Disc {
        kind: match kind {
            DiscKind::BluRay => v1::DiscKind::BluRay,
            DiscKind::Dvd => v1::DiscKind::Dvd,
        },
        file_count,
        duration_seconds: duration.map(|duration| duration.as_secs_f64()),
    }
}

fn movie_placement(movie: FileMovie) -> v1::Placement {
    let FileMovie {
        uuid,
        title,
        release_year,
        role,
        manifestation,
        part,
    } = movie;
    v1::Placement::Movie {
        uuid: uuid.0,
        title,
        year: release_year,
        role: role.map(file_role),
        manifestation,
        part,
    }
}

fn episode_placement(episode: FileEpisode) -> v1::Placement {
    let FileEpisode {
        series_uuid,
        series_name,
        season_number,
        episode_number,
        title,
    } = episode;
    v1::Placement::Episode {
        series_uuid: series_uuid.0,
        series_title: series_name,
        season: season_number,
        episode: episode_number,
        title,
    }
}

fn series_extra_placement(extra: FileSeriesExtra) -> v1::Placement {
    let FileSeriesExtra {
        series_uuid,
        series_name,
        disc,
    } = extra;
    if disc {
        v1::Placement::SeriesDisc {
            series_uuid: series_uuid.0,
            series_title: series_name,
        }
    } else {
        v1::Placement::SeriesExtra {
            series_uuid: series_uuid.0,
            series_title: series_name,
        }
    }
}

fn media_summary(media: FileMediaMeasurements) -> v1::Media {
    let duration_seconds = media.duration().map(|duration| duration.as_secs_f64());
    let FileMediaMeasurements {
        container,
        video,
        audio,
        text,
        unclassified: _,
        disc_image: _,
    } = media;
    let title_tag = container
        .as_ref()
        .and_then(|container| container.tags.as_ref())
        .and_then(|tags| tags.get("TITLE").or(tags.get("title")))
        .cloned();
    v1::Media {
        format: container.map(|container| container.format),
        duration_seconds,
        title_tag,
        video: video
            .into_iter()
            .map(|stream| v1::VideoStream {
                codec: stream.codec,
                width: stream.width,
                height: stream.height,
            })
            .collect(),
        audio: audio
            .into_iter()
            .map(|stream| v1::AudioStream {
                codec: stream.codec,
                channels: stream.channels,
                language: stream.language,
            })
            .collect(),
        subtitles: text
            .into_iter()
            .map(|stream| v1::SubtitleStream {
                codec: stream.codec,
                language: stream.language,
                forced: stream.is_forced,
            })
            .collect(),
    }
}

fn ranked_claim(claim: RankedClaimPredicate) -> v1::RankedClaim {
    let RankedClaimPredicate {
        pred,
        rank,
        active,
        source,
    } = claim;
    v1::RankedClaim {
        claim: claim_predicate(pred),
        source: match source {
            ClaimSource::Heuristic(heuristic) => v1::ClaimSource::Heuristic {
                name: snake_case(&format!("{heuristic:?}")),
            },
            ClaimSource::Assertion => v1::ClaimSource::Assertion,
            ClaimSource::KnowledgeBase { result } => v1::ClaimSource::KnowledgeBase { result },
        },
        rank: match rank {
            Rank::Number(rank) => v1::Rank::Number(rank),
            Rank::FoundByAssertion(bonus) => v1::Rank::FoundByAssertion(bonus),
            Rank::Assertion => v1::Rank::Assertion,
        },
        active,
    }
}

fn claim_predicate(pred: ClaimPredicate) -> v1::Claim {
    match pred {
        ClaimPredicate::Duration(duration) => v1::Claim::DurationSeconds(duration.as_secs()),
        ClaimPredicate::FileRole(role) => v1::Claim::FileRole(file_role(role)),
        ClaimPredicate::MovieExternalId(id) => v1::Claim::MovieExternalId(external_id(id)),
        ClaimPredicate::MovieTitle(title) => v1::Claim::MovieTitle(title),
        ClaimPredicate::MovieReleaseYear(year) => v1::Claim::MovieReleaseYear(year),
        ClaimPredicate::TvSeriesExternalId(id) => v1::Claim::TvSeriesExternalId(external_id(id)),
        ClaimPredicate::TvSeriesName(name) => v1::Claim::TvSeriesName(name),
        ClaimPredicate::TvSeriesYear(year) => v1::Claim::TvSeriesYear(year),
        ClaimPredicate::Season(season) => v1::Claim::Season(season),
        ClaimPredicate::EpisodeRange(range) => v1::Claim::EpisodeRange(range),
        ClaimPredicate::AbsoluteEpisodeRange(range) => v1::Claim::AbsoluteEpisodeRange(range),
        ClaimPredicate::Special => v1::Claim::Special,
        ClaimPredicate::Part(part) => v1::Claim::Part(part),
        ClaimPredicate::SpecialOf(name) => v1::Claim::SpecialOf(name),
        ClaimPredicate::ExtraOf(path) => v1::Claim::ExtraOf(path),
        ClaimPredicate::DiscOf(path) => v1::Claim::DiscOf(path),
        ClaimPredicate::Excluded => v1::Claim::Excluded,
        ClaimPredicate::Edition(edition) => v1::Claim::Edition(edition),
        ClaimPredicate::EpisodeTitle(title) => v1::Claim::EpisodeTitle(title),
        ClaimPredicate::WorkKind(kind) => v1::Claim::WorkKind(match kind {
            WorkKind::Movie => v1::WorkKind::Movie,
            WorkKind::TvSeries => v1::WorkKind::TvSeries,
        }),
    }
}

pub(super) fn file_role(role: FileRole) -> v1::FileRole {
    match role {
        FileRole::Main => v1::FileRole::Main,
        FileRole::Extra => v1::FileRole::Extra,
        FileRole::Sample => v1::FileRole::Sample,
        FileRole::Disc => v1::FileRole::Disc,
    }
}

pub(super) fn external_id(id: ExternalId) -> v1::ExternalId {
    match id {
        ExternalId::Imdb(id) => v1::ExternalId::Imdb(id),
        ExternalId::Tmdb(id) => v1::ExternalId::Tmdb(id),
        ExternalId::Tvdb(id) => v1::ExternalId::Tvdb(id),
    }
}

fn kb_query(query: KbQueryEvent) -> v1::KbQuery {
    let results = |results: Vec<KbSearchResult>| {
        results
            .into_iter()
            .map(|KbSearchResult { id, title, year }| v1::KbSearchResult {
                id: external_id(id),
                title,
                year,
            })
            .collect()
    };
    let lookup = |found: Option<KbLookupResult>| {
        found.map(
            |KbLookupResult {
                 id,
                 title,
                 year,
                 runtime_minutes,
             }| v1::KbLookupResult {
                id: external_id(id),
                title,
                year,
                runtime_minutes,
            },
        )
    };
    match query {
        KbQueryEvent::Find { id, found } => v1::KbQuery::Find {
            id: external_id(id),
            found: lookup(found),
        },
        KbQueryEvent::MovieSearch { query, results: r } => v1::KbQuery::MovieSearch {
            query,
            results: results(r),
        },
        KbQueryEvent::TvSeriesSearch { query, results: r } => v1::KbQuery::TvSeriesSearch {
            query,
            results: results(r),
        },
        KbQueryEvent::MovieDetails { id, found } => v1::KbQuery::MovieDetails {
            id: external_id(id),
            found: lookup(found),
        },
        KbQueryEvent::TvSeriesDetails { id, found } => v1::KbQuery::TvSeriesDetails {
            id: external_id(id),
            found: lookup(found),
        },
        KbQueryEvent::TvSeasonDetails { id, season } => v1::KbQuery::TvSeasonDetails {
            id: external_id(id),
            season,
        },
        KbQueryEvent::AlternativeTitles { id } => v1::KbQuery::AlternativeTitles {
            id: external_id(id),
        },
    }
}

pub(super) fn assertion(assertion: Assertion) -> v1::Assertion {
    let Assertion {
        uuid,
        claim,
        files,
        created_at,
        rationale,
        confidence,
    } = assertion;
    v1::Assertion {
        rationale,
        confidence,
        uuid: uuid.0,
        claim: claim_predicate(claim),
        files: files
            .into_iter()
            .map(
                |AssertedFile {
                     data_source_uuid,
                     data_source_name,
                     rel_path,
                     retracted_at,
                 }| v1::AssertedFile {
                    data_source: v1::DataSource {
                        uuid: data_source_uuid.0,
                        name: data_source_name,
                    },
                    path: rel_path.to_string_lossy().into_owned(),
                    retracted_at: retracted_at.map(|at| at.as_second()),
                },
            )
            .collect(),
        created_at: created_at.as_second(),
    }
}

/// E.g. `release_name` for `ReleaseName`.
fn snake_case(name: &str) -> String {
    let mut snake = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                snake.push('_');
            }
            snake.extend(c.to_lowercase());
        } else {
            snake.push(c);
        }
    }
    snake
}

fn top_directory(top_directory: TopDirectory, with_files: bool) -> v1::TopDirectory {
    let TopDirectory {
        data_source_uuid,
        data_source_name,
        root,
    } = top_directory;
    v1::TopDirectory {
        data_source: v1::DataSource {
            uuid: data_source_uuid.0,
            name: data_source_name,
        },
        root: release_directory(root, with_files),
    }
}

/// A directory, compactly unless `with_files` asks for everything:
/// only the directories that lead to the work's files are expanded, and their
/// other subdirectories fold everything below them.
fn release_directory(dir: Directory, with_files: bool) -> v1::Directory {
    let Directory {
        name,
        has_work_files,
        files,
        directories,
    } = dir;
    let directories = directories
        .into_iter()
        .map(|dir| {
            if with_files || leads_to_work_files(&dir) {
                release_directory(dir, with_files)
            } else {
                folded_directory(dir)
            }
        })
        .collect();
    v1::Directory {
        name,
        has_work_files,
        file_count: files.len(),
        placements: placement_counts(&files),
        files: with_files.then(|| files.into_iter().map(release_file).collect()),
        directories,
        folded_directories: 0,
    }
}

/// A directory with everything below it counted as its own.
fn folded_directory(dir: Directory) -> v1::Directory {
    fn fold(dir: Directory, files: &mut Vec<DirectoryFile>) -> usize {
        let Directory {
            files: own,
            directories,
            ..
        } = dir;
        files.extend(own);
        directories
            .into_iter()
            .map(|dir| 1 + fold(dir, files))
            .sum()
    }
    let name = dir.name.clone();
    let has_work_files = dir.has_work_files;
    let mut files = Vec::new();
    let folded_directories = fold(dir, &mut files);
    v1::Directory {
        name,
        has_work_files,
        file_count: files.len(),
        placements: placement_counts(&files),
        files: None,
        directories: Vec::new(),
        folded_directories,
    }
}

fn leads_to_work_files(dir: &Directory) -> bool {
    dir.has_work_files || dir.directories.iter().any(leads_to_work_files)
}

fn placement_counts(files: &[DirectoryFile]) -> Vec<v1::PlacementCount> {
    let mut counts: Vec<v1::PlacementCount> = Vec::new();
    for file in files {
        for group in placement_groups(file) {
            match counts.iter_mut().find(|count| count.placement == group) {
                Some(count) => count.files += 1,
                None => counts.push(v1::PlacementCount {
                    placement: group,
                    files: 1,
                }),
            }
        }
    }
    counts.sort_by_key(|count| std::cmp::Reverse(count.files));
    counts
}

/// Where a file went in the catalog. A file of several episodes of one
/// series is in its group once.
fn placement_groups(file: &DirectoryFile) -> Vec<v1::PlacementGroup> {
    let mut groups: Vec<v1::PlacementGroup> = Vec::new();
    let movies = file.movies.iter().map(|movie| v1::PlacementGroup::Movie {
        uuid: movie.uuid.0,
        title: movie.title.clone(),
        year: movie.release_year,
        role: movie.role.map(file_role),
    });
    let episodes = file
        .episodes
        .iter()
        .map(|episode| v1::PlacementGroup::Episodes {
            series_uuid: episode.series_uuid.0,
            series_title: episode.series_name.clone(),
        });
    let extras = file.series_extras.iter().map(|extra| {
        let (series_uuid, series_title) = (extra.series_uuid.0, extra.series_name.clone());
        if extra.disc {
            v1::PlacementGroup::SeriesDiscs {
                series_uuid,
                series_title,
            }
        } else {
            v1::PlacementGroup::SeriesExtras {
                series_uuid,
                series_title,
            }
        }
    });
    for group in movies.chain(episodes).chain(extras) {
        if !groups.contains(&group) {
            groups.push(group);
        }
    }
    if groups.is_empty() {
        groups.push(match file.unidentified_video {
            true => v1::PlacementGroup::UnidentifiedVideos,
            false => v1::PlacementGroup::Other,
        });
    }
    groups
}

fn release_file(file: DirectoryFile) -> v1::DirectoryFile {
    let DirectoryFile {
        rel_path,
        size,
        duration,
        unidentified_video: _,
        movies,
        episodes,
        series_extras,
    } = file;
    v1::DirectoryFile {
        name: rel_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        size_bytes: size,
        duration_seconds: duration.map(|duration| duration.as_secs_f64()),
        placements: movies
            .into_iter()
            .map(movie_placement)
            .chain(episodes.into_iter().map(episode_placement))
            .chain(series_extras.into_iter().map(series_extra_placement))
            .collect(),
    }
}

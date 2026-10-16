use super::{
    ApiError,
    evidence::{external_id, file_role},
    interpretation,
};
use crate::WebContext;
use axum::{
    Json,
    extract::{
        Path, Query, State,
        rejection::{PathRejection, QueryRejection},
    },
};
use uuid::Uuid;
use wanshitong_api::v1;
use wanshitong_app::commands;
use wanshitong_app::domain::{
    CatalogFile, CatalogWork, CatalogWorkKind, ChangesetUuid, ExternalId, FilePart,
    InterpretationUuid,
};
use wanshitong_app::queries::{self, FileChange, InterpretationDiff, WorkChange, WorkChangeKind};

/// Diff two interpretations' catalogs
#[utoipa::path(
    get,
    path = "/diff",
    tag = "Diff",
    params(v1::DiffQuery),
    responses((status = OK, body = v1::CatalogDiff), ApiError),
)]
pub(super) async fn interpretations(
    State(ctx): State<WebContext>,
    query: Result<Query<v1::DiffQuery>, QueryRejection>,
) -> Result<Json<v1::CatalogDiff>, ApiError> {
    let Query(v1::DiffQuery { base, other }) = query?;
    let diff = queries::diff_interpretations(
        &ctx.app,
        base.map(InterpretationUuid),
        InterpretationUuid(other),
    )
    .await?;
    Ok(Json(catalog_diff(diff)))
}

/// Preview what accepting a changeset would change
///
/// Interprets the changeset if its interpretation is out of date, which can
/// take a while.
#[utoipa::path(
    post,
    path = "/changesets/{uuid}/diff",
    tag = "Diff",
    params(("uuid" = Uuid, Path, description = "The changeset's.")),
    responses((status = OK, body = v1::CatalogDiff), ApiError),
)]
pub(super) async fn changeset(
    State(ctx): State<WebContext>,
    uuid: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<v1::CatalogDiff>, ApiError> {
    let Path(uuid) = uuid?;
    let diff = commands::diff_changeset(&ctx.app, ChangesetUuid(uuid)).await?;
    Ok(Json(catalog_diff(diff)))
}

fn catalog_diff(diff: InterpretationDiff) -> v1::CatalogDiff {
    let InterpretationDiff {
        base,
        other,
        changes,
        unidentified,
    } = diff;
    let files = |files: Vec<CatalogFile>| files.into_iter().map(diff_file).collect();
    v1::CatalogDiff {
        base: interpretation(&base),
        other: interpretation(&other),
        changes: changes.into_iter().map(work_change).collect(),
        unidentified: v1::UnidentifiedChange {
            left: files(unidentified.left),
            joined: files(unidentified.joined),
        },
    }
}

fn diff_file(file: CatalogFile) -> v1::DiffFile {
    v1::DiffFile {
        data_source: v1::DataSource {
            uuid: file.data_source_uuid.0,
            name: file.data_source_name,
        },
        path: file.rel_path.to_string_lossy().into_owned(),
    }
}

fn work_change(change: WorkChange) -> v1::WorkChange {
    let kind = match change.kind() {
        WorkChangeKind::Added => v1::WorkChangeKind::Added,
        WorkChangeKind::Removed => v1::WorkChangeKind::Removed,
        WorkChangeKind::Changed => v1::WorkChangeKind::Changed,
        WorkChangeKind::Split => v1::WorkChangeKind::Split,
        WorkChangeKind::Merged => v1::WorkChangeKind::Merged,
        WorkChangeKind::Regrouped => v1::WorkChangeKind::Regrouped,
    };
    let WorkChange {
        before,
        after,
        files,
        disc_files,
    } = change;
    v1::WorkChange {
        kind,
        before: before.into_iter().map(diff_work).collect(),
        after: after.into_iter().map(diff_work).collect(),
        files: files.into_iter().map(file_change).collect(),
        disc_files,
    }
}

fn diff_work(work: CatalogWork) -> v1::DiffWork {
    let CatalogWork {
        kind,
        uuid,
        title,
        year,
        tmdb_id,
        imdb_id,
        tvdb_id,
    } = work;
    let tmdb = tmdb_id.map(|id| ExternalId::Tmdb(id as u32));
    let imdb = imdb_id.map(ExternalId::Imdb);
    let tvdb = tvdb_id.map(|id| ExternalId::Tvdb(id as u32));
    v1::DiffWork {
        kind: match kind {
            CatalogWorkKind::Movie => v1::WorkKind::Movie,
            CatalogWorkKind::TvSeries => v1::WorkKind::TvSeries,
        },
        uuid,
        title,
        year,
        external_ids: [tmdb, imdb, tvdb]
            .into_iter()
            .flatten()
            .map(external_id)
            .collect(),
    }
}

fn file_change(file: FileChange) -> v1::FileChange {
    let FileChange {
        data_source_uuid,
        data_source_name,
        rel_path,
        before,
        after,
    } = file;
    let placements = |placements: Vec<(usize, FilePart)>| {
        placements
            .into_iter()
            .map(|(work, part)| v1::DiffPlacement {
                work,
                part: match part {
                    FilePart::Movie {
                        role,
                        manifestation,
                        part,
                    } => v1::FilePart::Movie {
                        role: role.map(file_role),
                        manifestation,
                        part,
                    },
                    FilePart::Episode { season, episode } => {
                        v1::FilePart::Episode { season, episode }
                    }
                    FilePart::SeriesExtra => v1::FilePart::SeriesExtra,
                    FilePart::SeriesDisc => v1::FilePart::SeriesDisc,
                },
            })
            .collect()
    };
    v1::FileChange {
        data_source: v1::DataSource {
            uuid: data_source_uuid.0,
            name: data_source_name,
        },
        path: rel_path.to_string_lossy().into_owned(),
        before: placements(before),
        after: placements(after),
    }
}

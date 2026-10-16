use super::{ApiError, evidence::assertion};
use crate::WebContext;
use axum::{
    Json,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
};
use std::time::Duration;
use uuid::Uuid;
use wanshitong_api::v1::{self, ErrorCode};
use wanshitong_app::commands::{self, NewAssertion, NewChangeset};
use wanshitong_app::domain::PathId;
use wanshitong_app::domain::{
    AssertionUuid, ChangesetDetails, ChangesetState, ChangesetUuid, ClaimPredicate, DataSourceUuid,
    ExternalId, FileRole, WorkKind,
};
use wanshitong_app::queries::{self, ChangesetSummary};

/// Open a changeset
///
/// Its assertions and retractions take effect only once it's accepted.
#[utoipa::path(
    post,
    path = "/changesets",
    tag = "Changesets",
    request_body = v1::NewChangeset,
    responses((status = OK, body = v1::Changeset), ApiError),
)]
pub(super) async fn create(
    State(ctx): State<WebContext>,
    body: Result<Json<v1::NewChangeset>, JsonRejection>,
) -> Result<Json<v1::Changeset>, ApiError> {
    let Json(v1::NewChangeset {
        author,
        description,
    }) = body?;
    let uuid = commands::create_changeset(
        &ctx.app,
        NewChangeset {
            author,
            description,
        },
    )
    .await?;
    get_changeset(&ctx, uuid).await.map(Json)
}

/// List changesets
#[utoipa::path(
    get,
    path = "/changesets",
    tag = "Changesets",
    params(v1::ChangesetsQuery),
    responses((status = OK, body = v1::Changesets), ApiError),
)]
pub(super) async fn list(
    State(ctx): State<WebContext>,
    query: Result<Query<v1::ChangesetsQuery>, QueryRejection>,
) -> Result<Json<v1::Changesets>, ApiError> {
    let Query(v1::ChangesetsQuery { state }) = query?;
    let changesets = queries::list_changesets(&ctx.app, state.map(domain_state)).await?;
    Ok(Json(v1::Changesets {
        changesets: changesets.into_iter().map(summary).collect(),
    }))
}

/// Get a changeset
#[utoipa::path(
    get,
    path = "/changesets/{uuid}",
    tag = "Changesets",
    params(("uuid" = Uuid, Path, description = "The changeset's.")),
    responses((status = OK, body = v1::Changeset), ApiError),
)]
pub(super) async fn get(
    State(ctx): State<WebContext>,
    uuid: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<v1::Changeset>, ApiError> {
    let Path(uuid) = uuid?;
    get_changeset(&ctx, ChangesetUuid(uuid)).await.map(Json)
}

/// Make assertions in an open changeset
#[utoipa::path(
    post,
    path = "/changesets/{uuid}/assertions",
    tag = "Changesets",
    params(("uuid" = Uuid, Path, description = "The changeset's.")),
    request_body = v1::NewAssertions,
    responses((status = OK, body = v1::CreatedAssertions), ApiError),
)]
pub(super) async fn assert(
    State(ctx): State<WebContext>,
    uuid: Result<Path<Uuid>, PathRejection>,
    body: Result<Json<v1::NewAssertions>, JsonRejection>,
) -> Result<Json<v1::CreatedAssertions>, ApiError> {
    let Path(uuid) = uuid?;
    let Json(v1::NewAssertions { assertions }) = body?;
    let assertions = assertions
        .into_iter()
        .map(
            |v1::NewAssertion {
                 files,
                 claim,
                 rationale,
                 confidence,
             }| {
                Ok(NewAssertion {
                    files: files
                        .into_iter()
                        .map(|v1::FileRef { data_source, path }| {
                            PathId::new(DataSourceUuid(data_source), path)
                        })
                        .collect(),
                    claim: domain_claim(claim)?,
                    rationale,
                    confidence,
                })
            },
        )
        .collect::<Result<_, ApiError>>()?;
    let created = commands::add_assertions(&ctx.app, ChangesetUuid(uuid), assertions).await?;
    Ok(Json(v1::CreatedAssertions {
        assertions: created.into_iter().map(|uuid| uuid.0).collect(),
    }))
}

/// Retract an assertion
///
/// Retracting an assertion that was made in this changeset just drops it.
/// Retracting an assertion in a separate accepted changeset will take effect
/// once this changeset is accepted.
#[utoipa::path(
    post,
    path = "/changesets/{uuid}/retractions",
    tag = "Changesets",
    params(("uuid" = Uuid, Path, description = "The changeset's.")),
    request_body = v1::NewRetraction,
    responses((status = OK, body = v1::Changeset), ApiError),
)]
pub(super) async fn retract(
    State(ctx): State<WebContext>,
    uuid: Result<Path<Uuid>, PathRejection>,
    body: Result<Json<v1::NewRetraction>, JsonRejection>,
) -> Result<Json<v1::Changeset>, ApiError> {
    let Path(uuid) = uuid?;
    let Json(v1::NewRetraction { assertion }) = body?;
    let changeset = ChangesetUuid(uuid);
    commands::retract_in_changeset(&ctx.app, changeset, AssertionUuid(assertion)).await?;
    get_changeset(&ctx, changeset).await.map(Json)
}

/// Accept a changeset
///
/// Applies its changes.
#[utoipa::path(
    post,
    path = "/changesets/{uuid}/accept",
    tag = "Changesets",
    params(("uuid" = Uuid, Path, description = "The changeset's.")),
    responses((status = OK, body = v1::AcceptedChangeset), ApiError),
)]
pub(super) async fn accept(
    State(ctx): State<WebContext>,
    uuid: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<v1::AcceptedChangeset>, ApiError> {
    let Path(uuid) = uuid?;
    let interpretation = commands::accept_changeset(&ctx.app, ChangesetUuid(uuid)).await?;
    Ok(Json(v1::AcceptedChangeset {
        interpretation: interpretation.0,
    }))
}

/// Discard a changeset
///
/// Drops its pending changes.
#[utoipa::path(
    post,
    path = "/changesets/{uuid}/discard",
    tag = "Changesets",
    params(("uuid" = Uuid, Path, description = "The changeset's.")),
    responses((status = OK, body = v1::Changeset), ApiError),
)]
pub(super) async fn discard(
    State(ctx): State<WebContext>,
    uuid: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<v1::Changeset>, ApiError> {
    let Path(uuid) = uuid?;
    let changeset = ChangesetUuid(uuid);
    commands::discard_changeset(&ctx.app, changeset).await?;
    get_changeset(&ctx, changeset).await.map(Json)
}

async fn get_changeset(ctx: &WebContext, uuid: ChangesetUuid) -> Result<v1::Changeset, ApiError> {
    let ChangesetDetails {
        uuid,
        author,
        description,
        state,
        created_at,
        closed_at,
        assertions,
        retractions,
    } = queries::get_changeset(&ctx.app, uuid)
        .await?
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, format!("no changeset {uuid}")))?;
    Ok(v1::Changeset {
        uuid: uuid.0,
        author,
        description,
        state: api_state(state),
        created_at: created_at.as_second(),
        closed_at: closed_at.map(|at| at.as_second()),
        assertions: assertions.into_iter().map(assertion).collect(),
        retractions: retractions.into_iter().map(|uuid| uuid.0).collect(),
    })
}

fn summary(changeset: ChangesetSummary) -> v1::ChangesetSummary {
    let ChangesetSummary {
        uuid,
        author,
        description,
        state,
        created_at,
        closed_at,
        assertion_count,
        retraction_count,
    } = changeset;
    v1::ChangesetSummary {
        uuid: uuid.0,
        author,
        description,
        state: api_state(state),
        created_at: created_at.as_second(),
        closed_at: closed_at.map(|at| at.as_second()),
        assertion_count,
        retraction_count,
    }
}

fn api_state(state: ChangesetState) -> v1::ChangesetState {
    match state {
        ChangesetState::Open => v1::ChangesetState::Open,
        ChangesetState::Accepted => v1::ChangesetState::Accepted,
        ChangesetState::Discarded => v1::ChangesetState::Discarded,
    }
}

fn domain_state(state: v1::ChangesetState) -> ChangesetState {
    match state {
        v1::ChangesetState::Open => ChangesetState::Open,
        v1::ChangesetState::Accepted => ChangesetState::Accepted,
        v1::ChangesetState::Discarded => ChangesetState::Discarded,
    }
}

/// The claim that the API's names, the reverse of what evidence shows. Discs
/// are found at ingest, so that a file is one of a disc's, or a disc's file,
/// isn't a claim to make, though old audit logs show such claims.
pub(super) fn domain_claim(claim: v1::Claim) -> Result<ClaimPredicate, ApiError> {
    let not_assertable = || {
        ApiError::new(
            ErrorCode::BadRequest,
            "a disc is found at ingest, not asserted: assert about its index file instead",
        )
    };
    Ok(match claim {
        v1::Claim::DurationSeconds(seconds) => {
            ClaimPredicate::Duration(Duration::from_secs(seconds))
        }
        v1::Claim::FileRole(v1::FileRole::Disc) | v1::Claim::DiscOf(_) => {
            return Err(not_assertable());
        }
        v1::Claim::FileRole(role) => ClaimPredicate::FileRole(domain_role(role)),
        v1::Claim::MovieExternalId(id) => ClaimPredicate::MovieExternalId(domain_id(id)),
        v1::Claim::MovieTitle(title) => ClaimPredicate::MovieTitle(title),
        v1::Claim::MovieReleaseYear(year) => ClaimPredicate::MovieReleaseYear(year),
        v1::Claim::TvSeriesExternalId(id) => ClaimPredicate::TvSeriesExternalId(domain_id(id)),
        v1::Claim::TvSeriesName(name) => ClaimPredicate::TvSeriesName(name),
        v1::Claim::TvSeriesYear(year) => ClaimPredicate::TvSeriesYear(year),
        v1::Claim::Season(season) => ClaimPredicate::Season(season),
        v1::Claim::EpisodeRange(range) => ClaimPredicate::EpisodeRange(range),
        v1::Claim::AbsoluteEpisodeRange(range) => ClaimPredicate::AbsoluteEpisodeRange(range),
        v1::Claim::Special => ClaimPredicate::Special,
        v1::Claim::Part(part) => ClaimPredicate::Part(part),
        v1::Claim::SpecialOf(name) => ClaimPredicate::SpecialOf(name),
        v1::Claim::ExtraOf(path) => ClaimPredicate::ExtraOf(path),
        v1::Claim::Excluded => ClaimPredicate::Excluded,
        v1::Claim::Edition(edition) => ClaimPredicate::Edition(edition),
        v1::Claim::EpisodeTitle(title) => ClaimPredicate::EpisodeTitle(title),
        v1::Claim::WorkKind(kind) => ClaimPredicate::WorkKind(match kind {
            v1::WorkKind::Movie => WorkKind::Movie,
            v1::WorkKind::TvSeries => WorkKind::TvSeries,
        }),
    })
}

fn domain_role(role: v1::FileRole) -> FileRole {
    match role {
        v1::FileRole::Main => FileRole::Main,
        v1::FileRole::Extra => FileRole::Extra,
        v1::FileRole::Sample => FileRole::Sample,
        v1::FileRole::Disc => FileRole::Disc,
    }
}

fn domain_id(id: v1::ExternalId) -> ExternalId {
    match id {
        v1::ExternalId::Imdb(id) => ExternalId::Imdb(id),
        v1::ExternalId::Tmdb(id) => ExternalId::Tmdb(id),
        v1::ExternalId::Tvdb(id) => ExternalId::Tvdb(id),
    }
}

//! The external JSON API, `/api/v1`. Its types are in `wanshitong-api`.

pub(crate) mod assertions;
mod changesets;
mod diff;
mod evidence;
mod health;
mod kb;
mod marks;
mod unidentified;

use crate::WebContext;
use crate::error::{app_error_message, app_error_status};
use axum::{
    Json,
    extract::rejection::{JsonRejection, PathRejection, QueryRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::collections::BTreeMap;
use utoipa::{
    IntoResponses, OpenApi,
    openapi::{Ref, RefOr, ResponseBuilder, content::Content, response},
};
use utoipa_axum::{router::OpenApiRouter, routes};
use wanshitong_api::v1::{self, ErrorCode, ErrorResponse};
use wanshitong_app::{
    AppError, AppErrorContext,
    interpretations::{InterpretationState, InterpretationStatus},
    report_error,
};

/// The API's OpenAPI document, but for its paths, which come from the
/// handlers' `#[utoipa::path]`s as [`router`] routes them.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Wan Shi Tong",
        description = "WST's external JSON API.",
    ),
    // For [`ApiError`]'s responses, which refer to it by name.
    components(schemas(ErrorResponse)),
    tags(
        (name = "Evidence", description = "What WST knows about files and works."),
        (name = "Changesets", description = "Assertions and retractions."),
        (name = "Assertions", description = "The accepted assertions in effect."),
        (name = "Diff", description = "How catalogs differ, e.g. what accepting a changeset would change."),
        (name = "Knowledge base", description = "Make queries to external knowledge bases."),
        (name = "Marks", description = "Files marked as needing correction."),
        (name = "Service", description = "The service itself."),
    ),
)]
pub(crate) struct ApiDoc;

/// The routes, documented: `routes!` adds each handler's `#[utoipa::path]` to
/// the document, so the two can't drift apart.
pub(crate) fn router(state: WebContext) -> OpenApiRouter {
    OpenApiRouter::new()
        .routes(routes!(health::get))
        .routes(routes!(unidentified::list))
        .routes(routes!(evidence::get_work))
        .routes(routes!(evidence::get_file))
        .routes(routes!(changesets::list, changesets::create))
        .routes(routes!(changesets::get))
        .routes(routes!(changesets::assert))
        .routes(routes!(changesets::retract))
        .routes(routes!(changesets::accept))
        .routes(routes!(changesets::discard))
        .routes(routes!(assertions::list))
        .routes(routes!(assertions::import))
        .routes(routes!(diff::changeset))
        .routes(routes!(diff::interpretations))
        .routes(routes!(kb::search))
        .routes(routes!(kb::movie))
        .routes(routes!(kb::tv_series))
        .routes(routes!(marks::list))
        .fallback(not_found)
        .with_state(state)
}

async fn not_found() -> ApiError {
    ApiError::new(ErrorCode::NotFound, "no such endpoint")
}

/// An error, as the API reports it.
pub(crate) struct ApiError {
    status: StatusCode,
    code: ErrorCode,
    message: String,
}

impl ApiError {
    pub(crate) fn status(&self) -> StatusCode {
        self.status
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        let status = match code {
            ErrorCode::BadRequest => StatusCode::BAD_REQUEST,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::Conflict | ErrorCode::NoCatalog => StatusCode::CONFLICT,
            ErrorCode::NoKnowledgeBase => StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::KnowledgeBaseFailed => StatusCode::BAD_GATEWAY,
            ErrorCode::Internal | ErrorCode::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

/// Any endpoint's errors, as `#[utoipa::path]` lists them.
impl IntoResponses for ApiError {
    fn responses() -> BTreeMap<String, RefOr<response::Response>> {
        let error = |description: &str| {
            ResponseBuilder::new()
                .description(description)
                .content(
                    "application/json",
                    Content::new(Some(Ref::from_schema_name("ErrorResponse"))),
                )
                .build()
                .into()
        };
        BTreeMap::from([
            (
                "4XX".to_owned(),
                error("The request can't be done, e.g. `not_found` or `conflict`."),
            ),
            (
                "5XX".to_owned(),
                error("WST, or a knowledge base, failed to do it."),
            ),
        ])
    }
}

impl From<AppError> for ApiError {
    /// With the web UI's status for the error, and a code for the status
    /// unless the error has its own.
    fn from(error: AppError) -> Self {
        let status = app_error_status(&error);
        let code = match (error.context(), status) {
            (AppErrorContext::NoCatalog, _) => ErrorCode::NoCatalog,
            (AppErrorContext::NoKnowledgeBase, _) => ErrorCode::NoKnowledgeBase,
            (AppErrorContext::KnowledgeBase(_), _) => ErrorCode::KnowledgeBaseFailed,
            (_, StatusCode::BAD_REQUEST) => ErrorCode::BadRequest,
            (_, StatusCode::NOT_FOUND) => ErrorCode::NotFound,
            (_, StatusCode::CONFLICT) => ErrorCode::Conflict,
            _ => ErrorCode::Internal,
        };
        if status.is_server_error() {
            report_error!(error, "API request failed");
        }
        Self {
            status,
            code,
            message: app_error_message(&error),
        }
    }
}

impl From<PathRejection> for ApiError {
    fn from(rejection: PathRejection) -> Self {
        Self::new(ErrorCode::BadRequest, rejection.body_text())
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        Self::new(ErrorCode::BadRequest, rejection.body_text())
    }
}

impl From<QueryRejection> for ApiError {
    fn from(rejection: QueryRejection) -> Self {
        Self::new(ErrorCode::BadRequest, rejection.body_text())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let Self {
            status,
            code,
            message,
        } = self;
        let body = ErrorResponse {
            error: v1::Error { code, message },
        };
        (status, Json(body)).into_response()
    }
}

fn interpretation(details: &InterpretationStatus) -> v1::Interpretation {
    let interpretation = &details.interpretation;
    v1::Interpretation {
        uuid: interpretation.uuid.0,
        name: interpretation.name.clone(),
        canonical: interpretation.is_canonical,
        state: match details.state {
            InterpretationState::Running => v1::InterpretationState::Running,
            InterpretationState::Interrupted => v1::InterpretationState::Interrupted,
            InterpretationState::Failed => v1::InterpretationState::Failed,
            InterpretationState::Succeeded => v1::InterpretationState::Succeeded,
        },
    }
}

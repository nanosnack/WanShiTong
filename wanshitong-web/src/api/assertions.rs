//! The assertions in effect, and importing an export of them: the JSON API's
//! endpoints, and what the Assertions page's Export and Import share with
//! them.

use super::{ApiError, changesets::domain_claim, evidence::assertion};
use crate::WebContext;
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
};
use jiff::Timestamp;
use wanshitong_api::v1;
use wanshitong_app::commands::{
    AssertionImport, ImportedAssertion, ImportedAssertions, ImportedFile, import_assertions,
};
use wanshitong_app::domain::DataSourceUuid;
use wanshitong_app::{AppContext, AppError, queries};

/// List the assertions in effect
///
/// The accepted assertions that new interpretations include, newest first:
/// the same as the Corrections section's assertions page.
#[utoipa::path(
    get,
    path = "/assertions",
    tag = "Assertions",
    responses((status = OK, body = v1::ActiveAssertions), ApiError),
)]
pub(super) async fn list(
    State(ctx): State<WebContext>,
) -> Result<Json<v1::ActiveAssertions>, ApiError> {
    Ok(Json(active_assertions(&ctx.app).await?))
}

/// Import assertions
///
/// Makes an export's assertions again in a new changeset, all or none of
/// them, which is left open to review and accept. Each file's data source is
/// matched by UUID, or else by name.
#[utoipa::path(
    post,
    path = "/assertions/import",
    tag = "Assertions",
    request_body = v1::ImportAssertions,
    responses((status = OK, body = v1::ImportedAssertions), ApiError),
)]
pub(super) async fn import(
    State(ctx): State<WebContext>,
    body: Result<Json<v1::ImportAssertions>, JsonRejection>,
) -> Result<Json<v1::ImportedAssertions>, ApiError> {
    let Json(body) = body?;
    let ImportedAssertions {
        changeset,
        assertions,
    } = import_exported(&ctx.app, body).await?;
    Ok(Json(v1::ImportedAssertions {
        changeset: changeset.0,
        assertions: assertions.into_iter().map(|uuid| uuid.0).collect(),
    }))
}

/// The assertions in effect, as the API lists them, and as an export file
/// holds them.
pub(crate) async fn active_assertions(app: &AppContext) -> Result<v1::ActiveAssertions, AppError> {
    let assertions = queries::list_active_assertions(app).await?;
    Ok(v1::ActiveAssertions {
        assertions: assertions.into_iter().map(assertion).collect(),
    })
}

/// Makes an export's assertions again, as the API's names them. See
/// [`import_assertions`].
pub(crate) async fn import_exported(
    app: &AppContext,
    import: v1::ImportAssertions,
) -> Result<ImportedAssertions, ApiError> {
    let v1::ImportAssertions {
        author,
        description,
        assertions,
    } = import;
    let assertions = assertions
        .into_iter()
        // Exports list them newest first, and the import makes them oldest
        // first, so reversing keeps the order of those made together.
        .rev()
        .map(
            |v1::Assertion {
                 uuid: _,
                 claim,
                 files,
                 created_at,
                 rationale,
                 confidence,
             }| {
                Ok(ImportedAssertion {
                    files: files
                        .into_iter()
                        .map(
                            |v1::AssertedFile {
                                 data_source,
                                 path,
                                 retracted_at: _,
                             }| ImportedFile {
                                data_source_uuid: DataSourceUuid(data_source.uuid),
                                data_source_name: data_source.name,
                                rel_path: path.into(),
                            },
                        )
                        .collect(),
                    claim: domain_claim(claim)?,
                    rationale,
                    confidence,
                    created_at: Timestamp::from_second(created_at).map_err(|_| {
                        ApiError::new(
                            v1::ErrorCode::BadRequest,
                            format!("{created_at} isn't a UNIX timestamp"),
                        )
                    })?,
                })
            },
        )
        .collect::<Result<_, ApiError>>()?;
    Ok(import_assertions(
        app,
        AssertionImport {
            author,
            description,
            assertions,
        },
    )
    .await?)
}

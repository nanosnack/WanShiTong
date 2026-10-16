use crate::WST_VERSION;
use axum::Json;
use wanshitong_api::v1::{Health, HealthStatus};

/// Check that the service is up, and which version it runs
///
/// Answers as soon as the service accepts requests. The database is in the
/// process, so there is nothing else to check.
#[utoipa::path(
    get,
    path = "/health",
    tag = "Service",
    responses((status = OK, body = Health)),
)]
pub(super) async fn get() -> Json<Health> {
    Json(Health {
        status: HealthStatus::Ok,
        version: WST_VERSION.to_owned(),
    })
}

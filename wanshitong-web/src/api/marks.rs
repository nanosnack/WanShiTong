use super::ApiError;
use crate::WebContext;
use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};
use wanshitong_api::v1::{self, DataSource};
use wanshitong_app::domain::{Mark, MarkProgress, MarkedFile};
use wanshitong_app::queries;

/// List marks
///
/// The same marks as the Corrections page.
#[utoipa::path(
    get,
    path = "/marks",
    tag = "Marks",
    params(v1::MarksQuery),
    responses((status = OK, body = v1::Marks), ApiError),
)]
pub(super) async fn list(
    State(ctx): State<WebContext>,
    query: Result<Query<v1::MarksQuery>, QueryRejection>,
) -> Result<Json<v1::Marks>, ApiError> {
    let Query(v1::MarksQuery { done }) = query?;
    let marks = queries::list_marks(&ctx.app).await?;
    Ok(Json(v1::Marks {
        marks: marks
            .into_iter()
            .filter(|mark| done || mark.progress != MarkProgress::Done)
            .map(mark)
            .collect(),
    }))
}

fn mark(mark: Mark) -> v1::Mark {
    let Mark {
        uuid,
        label,
        note,
        created_at,
        files,
        progress,
    } = mark;
    v1::Mark {
        uuid: uuid.0,
        label,
        note,
        created_at: created_at.as_second(),
        progress: match progress {
            MarkProgress::Open => v1::MarkProgress::Open,
            MarkProgress::Pending => v1::MarkProgress::Pending,
            MarkProgress::Done => v1::MarkProgress::Done,
        },
        files: files
            .into_iter()
            .map(
                |MarkedFile {
                     data_source_uuid,
                     data_source_name,
                     rel_path,
                 }| v1::MarkedFile {
                    data_source: DataSource {
                        uuid: data_source_uuid.0,
                        name: data_source_name,
                    },
                    path: rel_path.to_string_lossy().into_owned(),
                },
            )
            .collect(),
    }
}

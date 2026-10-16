use super::{ApiError, evidence::v1_disc, interpretation};
use crate::{
    WebContext,
    cursor::{decode_cursor, encode_cursor},
};
use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};
use serde::{Deserialize, Serialize};
use wanshitong_api::v1::{self, DataSource, ErrorCode, UnidentifiedQuery, UnidentifiedVideos};
use wanshitong_app::{
    domain::{FileSummary, InterpretationUuid},
    queries::{self, FileCursor, FileSearchQuery},
};

const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 500;

/// A page's position in one interpretation's unidentified videos, which the
/// next page must read too.
#[derive(Deserialize, Serialize)]
struct Cursor {
    interpretation: InterpretationUuid,
    position: FileCursor,
}

/// List unidentified media files
#[utoipa::path(
    get,
    path = "/unidentified",
    tag = "Evidence",
    params(UnidentifiedQuery),
    responses((status = OK, body = UnidentifiedVideos), ApiError),
)]
pub(super) async fn list(
    State(ctx): State<WebContext>,
    query: Result<Query<UnidentifiedQuery>, QueryRejection>,
) -> Result<Json<UnidentifiedVideos>, ApiError> {
    let Query(UnidentifiedQuery {
        interpretation: requested,
        name,
        cursor,
        limit,
    }) = query?;
    let cursor = match cursor {
        Some(cursor) => Some(
            decode_cursor::<Cursor>(&cursor)
                .ok_or_else(|| ApiError::new(ErrorCode::BadRequest, "invalid cursor"))?,
        ),
        None => None,
    };
    let requested = requested.map(InterpretationUuid);
    let requested = match (&cursor, requested) {
        (Some(cursor), Some(requested)) if cursor.interpretation != requested => {
            return Err(ApiError::new(
                ErrorCode::BadRequest,
                "the cursor is from another interpretation",
            ));
        }
        (Some(cursor), _) => Some(cursor.interpretation),
        (None, requested) => requested,
    };
    let details = queries::get_interpretation_or_canonical(&ctx.app, requested).await?;
    let page = queries::list_files_in(
        &ctx.app,
        details.interpretation.id,
        &FileSearchQuery {
            name,
            folder: None,
            work: None,
            identified: false,
            non_videos: false,
            cursor: cursor.map(|cursor| cursor.position),
            limit: limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT) as i64,
        },
    )
    .await?;

    Ok(Json(UnidentifiedVideos {
        interpretation: interpretation(&details),
        files: page.files.into_iter().map(file_summary).collect(),
        next_cursor: page.next_cursor.map(|position| {
            encode_cursor(&Cursor {
                interpretation: details.interpretation.uuid,
                position,
            })
        }),
    }))
}

fn file_summary(file: FileSummary) -> v1::FileSummary {
    let FileSummary {
        data_source_uuid,
        data_source_name,
        rel_path,
        size,
        disc,
    } = file;
    v1::FileSummary {
        data_source: DataSource {
            uuid: data_source_uuid.0,
            name: data_source_name,
        },
        path: rel_path,
        size_bytes: size,
        disc: disc.map(|disc| v1_disc(disc.kind, disc.file_count, disc.duration)),
    }
}

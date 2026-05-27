use axum::{
    extract::{Query, State},
    Json,
};
use std::sync::Arc;

use crate::app::AppState;
use crate::dto::ok;
use crate::dto::ApiResult;
use crate::middleware::AdminUser;
use crate::models::{GenerateDownloadUrlDto, GenerateUploadUrlDto, ListObjectsQuery};
use crate::openapi::responses::{
    ErrorEnvelope, ListObjectsEnvelope, PresignedDownloadUrlEnvelope, PresignedUploadUrlEnvelope,
};
use crate::services::storage_service::{
    require_storage, ListObjectsResponse, PresignedDownloadUrlResponse, PresignedUploadUrlResponse,
};

#[utoipa::path(
    post,
    path = "/storage/upload-url",
    request_body = GenerateUploadUrlDto,
    responses(
        (status = 200, description = "Generate upload URL", body = PresignedUploadUrlEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "storage"
)]
pub async fn generate_upload_url(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<GenerateUploadUrlDto>,
) -> ApiResult<PresignedUploadUrlResponse> {
    state
        .files
        .validate_upload_content_length(dto.content_length)?;

    let storage = require_storage(&state.storage)?;
    let result = storage
        .generate_upload_url(
            &dto.key,
            dto.content_type.as_deref(),
            dto.expires_in,
        )
        .await?;
    ok(result)
}

#[utoipa::path(
    post,
    path = "/storage/download-url",
    request_body = GenerateDownloadUrlDto,
    responses(
        (status = 200, description = "Generate download URL", body = PresignedDownloadUrlEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "storage"
)]
pub async fn generate_download_url(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<GenerateDownloadUrlDto>,
) -> ApiResult<PresignedDownloadUrlResponse> {
    let storage = require_storage(&state.storage)?;
    let result = storage
        .generate_download_url(&dto.key, dto.expires_in)
        .await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/storage/list",
    responses(
        (status = 200, description = "List storage objects", body = ListObjectsEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "storage"
)]
pub async fn list_objects(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListObjectsQuery>,
) -> ApiResult<ListObjectsResponse> {
    let storage = require_storage(&state.storage)?;
    let result = storage
        .list_objects(
            query.prefix.as_deref(),
            query.max_keys,
            query.continuation_token.as_deref(),
        )
        .await?;
    ok(result)
}

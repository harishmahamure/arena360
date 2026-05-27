use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::middleware::AdminUser;
use crate::models::{
    CreateFileDto, FileFilterDto, FileRecord, FileWithDownloadUrlDto, StorageStatsDto,
    UpdateFileDto,
};
use crate::openapi::responses::{
    ErrorEnvelope, FileEnvelope, FilePaginationEnvelope, FileWithDownloadUrlEnvelope,
    StorageStatsEnvelope,
};

#[derive(Debug, serde::Deserialize, ToSchema, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct DownloadUrlQuery {
    pub expires_in: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/files",
    responses(
        (status = 200, description = "List files", body = FilePaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "files"
)]
pub async fn list_files(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<FileFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<FileRecord>> {
    let result = state.files.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/files/stats",
    responses(
        (status = 200, description = "Get file storage statistics", body = StorageStatsEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "files"
)]
pub async fn get_storage_stats(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<StorageStatsDto> {
    let stats = state.files.get_storage_stats().await?;
    ok(stats)
}

#[utoipa::path(
    get,
    path = "/files/{id}",
    params(
        ("id" = Uuid, Path, description = "File ID"),
    ),
    responses(
        (status = 200, description = "Get file", body = FileEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "files"
)]
pub async fn get_file(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<FileRecord> {
    let file = state.files.get_by_id(id).await?;
    ok(file)
}

#[utoipa::path(
    post,
    path = "/files",
    request_body = CreateFileDto,
    responses(
        (status = 201, description = "Create file", body = FileEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "files"
)]
pub async fn create_file(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateFileDto>,
) -> ApiResult<FileRecord> {
    let uploaded_by = Uuid::parse_str(&claims.userId).ok();
    let file = state.files.create(dto, uploaded_by).await?;
    created(file)
}

#[utoipa::path(
    put,
    path = "/files/{id}",
    params(
        ("id" = Uuid, Path, description = "File ID"),
    ),
    request_body = UpdateFileDto,
    responses(
        (status = 200, description = "Update file", body = FileEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "files"
)]
pub async fn update_file(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateFileDto>,
) -> ApiResult<FileRecord> {
    let file = state.files.update(id, dto).await?;
    ok(file)
}

#[utoipa::path(
    delete,
    path = "/files/{id}",
    params(
        ("id" = Uuid, Path, description = "File ID"),
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "files"
)]
pub async fn delete_file(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, crate::error::AppError> {
    state.files.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/files/{id}/download-url",
    params(
        ("id" = Uuid, Path, description = "File ID"),
        ("expires_in" = Option<u64>, Query, description = "URL expiry in seconds"),
    ),
    responses(
        (status = 200, description = "Get file download URL", body = FileWithDownloadUrlEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "files"
)]
pub async fn get_file_download_url(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Query(query): Query<DownloadUrlQuery>,
) -> ApiResult<FileWithDownloadUrlDto> {
    let result = state
        .files
        .get_with_download_url(id, query.expires_in)
        .await?;
    ok(result)
}

#[utoipa::path(
    put,
    path = "/files/{id}/archive",
    params(
        ("id" = Uuid, Path, description = "File ID"),
    ),
    responses(
        (status = 200, description = "Archive file", body = FileEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "files"
)]
pub async fn archive_file(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<FileRecord> {
    let file = state.files.archive(id).await?;
    ok(file)
}

#[utoipa::path(
    put,
    path = "/files/{id}/activate",
    params(
        ("id" = Uuid, Path, description = "File ID"),
    ),
    responses(
        (status = 200, description = "Activate file", body = FileEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "files"
)]
pub async fn activate_file(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<FileRecord> {
    let file = state.files.activate(id).await?;
    ok(file)
}

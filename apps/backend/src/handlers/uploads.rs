use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::app::AppState;
use crate::dto::{ok, ApiResult};
use crate::middleware::AdminUser;
use crate::openapi::responses::{ErrorEnvelope, PresignResponseEnvelope};
use crate::services::storage_service::PresignedUpload;

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, ToSchema)]
pub struct PresignRequest {
    /// Original file name; used to derive the object key and extension.
    pub fileName: String,
    /// MIME type the client will upload with (e.g. `image/png`, `video/mp4`).
    pub contentType: Option<String>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, ToSchema)]
pub struct PresignResponse {
    /// Presigned PUT URL the client uploads the bytes to.
    pub uploadUrl: String,
    /// Stable public URL to store on the game record.
    pub publicUrl: String,
    /// Object key in the bucket.
    pub key: String,
}

impl From<PresignedUpload> for PresignResponse {
    fn from(p: PresignedUpload) -> Self {
        Self {
            uploadUrl: p.upload_url,
            publicUrl: p.public_url,
            key: p.key,
        }
    }
}

#[utoipa::path(
    post,
    path = "/uploads/presign",
    request_body = PresignRequest,
    responses(
        (status = 200, description = "Presigned upload issued", body = PresignResponseEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Storage not configured", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "uploads"
)]
pub async fn presign_upload(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(req): Json<PresignRequest>,
) -> ApiResult<PresignResponse> {
    let key = crate::services::StorageService::game_asset_key(&req.fileName);
    let presigned = state.storage.presign_put(&key, req.contentType.as_deref())?;
    ok(PresignResponse::from(presigned))
}

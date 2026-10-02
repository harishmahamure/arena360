use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::access::has;
use crate::app::AppState;
use crate::dto::{ok, ApiResult, JwtUserClaims};
use crate::error::AppError;
use crate::middleware::AdminOrStaff;
use crate::openapi::responses::{ErrorEnvelope, PresignResponseEnvelope};
use crate::services::storage_service::PresignedUpload;
use crate::services::StorageService;

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, ToSchema)]
pub struct PresignRequest {
    /// Original file name; used to derive the object key and extension.
    pub fileName: String,
    /// MIME type the client will upload with (e.g. `image/webp`, `video/webm`).
    pub contentType: Option<String>,
    /// `avatar` (own profile photo, WebP), `branding` (requires settings:write), or catalog media.
    pub purpose: Option<String>,
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

const CATALOG_UPLOAD_PERMISSIONS: [&str; 3] =
    ["products:write", "expenses:write", "procurement:write"];

/// Object key for an upload, enforcing who may upload for each purpose.
fn upload_key(claims: &JwtUserClaims, req: &PresignRequest) -> Result<String, AppError> {
    match req.purpose.as_deref() {
        Some("avatar") => {
            if req.contentType.as_deref() != Some("image/webp") {
                return Err(AppError::BadRequest(
                    "Profile photos must be uploaded as image/webp".into(),
                ));
            }
            Ok(StorageService::asset_key(
                &format!("avatars/{}", claims.userId),
                &req.fileName,
            ))
        }
        Some("branding") if has(claims, "settings:write") => {
            Ok(StorageService::asset_key("branding", &req.fileName))
        }
        Some("branding") => Err(AppError::Forbidden(
            "Uploading branding requires settings:write".into(),
        )),
        _ if claims.roles.iter().any(|role| role == "admin")
            && CATALOG_UPLOAD_PERMISSIONS.iter().any(|p| has(claims, p)) =>
        {
            Ok(StorageService::game_asset_key(&req.fileName))
        }
        _ => Err(AppError::Forbidden(
            "Uploading catalog media requires an administrator with catalog write access".into(),
        )),
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
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(req): Json<PresignRequest>,
) -> ApiResult<PresignResponse> {
    let key = upload_key(&claims, &req)?;
    let presigned = state
        .storage
        .presign_put(&key, req.contentType.as_deref())?;
    ok(PresignResponse::from(presigned))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(roles: &[&str], permissions: &[&str]) -> JwtUserClaims {
        serde_json::from_value(serde_json::json!({
            "sub": "u1", "userId": "u1", "roles": roles, "tenantId": "org", "orgIds": ["org"],
            "allowedTenants": ["org"], "permissions": permissions,
            "iss": "gamezone", "aud": "gamezone", "appId": "test"
        }))
        .unwrap()
    }

    fn request(purpose: Option<&str>, content_type: &str) -> PresignRequest {
        PresignRequest {
            fileName: "photo.webp".into(),
            contentType: Some(content_type.into()),
            purpose: purpose.map(Into::into),
        }
    }

    #[test]
    fn staff_may_upload_only_their_own_webp_avatar() {
        let staff = claims(&["staff"], &[]);
        let key = upload_key(&staff, &request(Some("avatar"), "image/webp")).unwrap();
        assert!(key.starts_with("avatars/u1/"));
        assert!(upload_key(&staff, &request(Some("avatar"), "image/png")).is_err());
        assert!(upload_key(&staff, &request(Some("branding"), "image/webp")).is_err());
        assert!(upload_key(&staff, &request(None, "image/webp")).is_err());
    }

    #[test]
    fn branding_and_catalog_uploads_keep_their_permissions() {
        let settings = claims(&["staff"], &["settings:write"]);
        assert!(
            upload_key(&settings, &request(Some("branding"), "image/webp"))
                .unwrap()
                .starts_with("branding/")
        );
        let admin = claims(&["admin"], &["products:write"]);
        assert!(upload_key(&admin, &request(None, "image/webp"))
            .unwrap()
            .starts_with("games/"));
    }
}

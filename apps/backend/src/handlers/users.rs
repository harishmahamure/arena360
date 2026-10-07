use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{ok, ApiResult, ChangePasswordDto};
use crate::middleware::{AdminOrStaff, AdminUser};
use crate::models::{TotpSetupResponseDto, UpdateUserDto, User, UserFilterDto, VerifyTotpSetupDto};
use crate::openapi::responses::{ErrorEnvelope, UserEnvelope, UserPaginationEnvelope};

fn identities(
    state: &AppState,
) -> Result<crate::control::identity::IdentityRepository, crate::error::AppError> {
    let pool = state
        .control_db
        .clone()
        .ok_or_else(|| crate::error::AppError::Api {
            code: "CONTROL_AUTH_UNAVAILABLE".into(),
            status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
            details: None,
        })?;
    Ok(crate::control::identity::IdentityRepository::new(pool))
}
async fn sync_identity(
    state: &AppState,
    db: Arc<crate::tenancy::TenantDb>,
) -> Result<(), crate::error::AppError> {
    let pool = state
        .control_db
        .as_ref()
        .ok_or_else(|| crate::error::AppError::Api {
            code: "CONTROL_AUTH_UNAVAILABLE".into(),
            status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
            details: None,
        })?;
    crate::control::staff_projection::sync_tenant(pool, db).await
}

#[utoipa::path(
    get,
    path = "/users",
    responses(
        (status = 200, description = "List users", body = UserPaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn list_users(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(mut filters): Query<UserFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<User>> {
    if !crate::access::has(&claims, "team:read") {
        filters.role = Some("player".into());
    }
    let mut result = state
        .users
        .list_tenant(state.business_db(&claims).await?, filters)
        .await?;
    if result
        .data
        .iter()
        .any(|u| u.role.as_deref() != Some("player"))
    {
        identities(&state)?
            .fill_mfa_status(
                state.business_db(&claims).await?.tenant_id(),
                &mut result.data,
            )
            .await?;
    }
    ok(result)
}

#[utoipa::path(
    get,
    path = "/users/{id}",
    params(
        ("id" = Uuid, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "Get user", body = UserEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn get_user(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<User> {
    let mut user = state
        .users
        .get_by_id_tenant(state.business_db(&claims).await?, id)
        .await?;
    if user.role.as_deref() != Some("player") && !crate::access::has(&claims, "team:read") {
        return Err(crate::error::AppError::Forbidden(
            "Team read permission required".into(),
        ));
    }
    if user.role.as_deref() != Some("player") {
        identities(&state)?
            .fill_mfa_status(
                state.business_db(&claims).await?.tenant_id(),
                std::slice::from_mut(&mut user),
            )
            .await?;
    }
    ok(user)
}

#[utoipa::path(
    put,
    path = "/users/{id}",
    params(
        ("id" = Uuid, Path, description = "User ID"),
    ),
    request_body = UpdateUserDto,
    responses(
        (status = 200, description = "Update user", body = UserEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn update_user(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateUserDto>,
) -> ApiResult<User> {
    let target = state
        .users
        .get_by_id_tenant(state.business_db(&claims).await?, id)
        .await?;
    if dto
        .role
        .as_deref()
        .is_some_and(|r| Some(r) != target.role.as_deref())
    {
        return Err(crate::error::AppError::Forbidden(
            "Manage panel roles through Access management; account types cannot be changed here"
                .into(),
        ));
    }
    if target.role.as_deref() != Some("player") {
        if dto.is_active.is_some() {
            return Err(crate::error::AppError::Forbidden(
                "Manage team access through Access management".into(),
            ));
        }
        if !crate::access::has(&claims, "team:write") {
            return Err(crate::error::AppError::Forbidden(
                "Team write permission required".into(),
            ));
        }
    }
    let db = state.business_db(&claims).await?;
    let user = if target.role.as_deref() == Some("player") {
        state
            .users
            .update_tenant(db, id, dto, claims.user_id_uuid())
            .await?
    } else {
        identities(&state)?
            .update_profile(db.tenant_id(), id, dto)
            .await?;
        sync_identity(&state, db.clone()).await?;
        state.users.get_by_id_tenant(db, id).await?
    };
    ok(user)
}

#[utoipa::path(
    put,
    path = "/users/{id}/password",
    params(
        ("id" = Uuid, Path, description = "User ID"),
    ),
    request_body = ChangePasswordDto,
    responses(
        (status = 200, description = "Password changed", body = serde_json::Value),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn change_password(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<ChangePasswordDto>,
) -> ApiResult<serde_json::Value> {
    let db = state.business_db(&claims).await?;
    let target = state.users.get_by_id_tenant(db.clone(), id).await?;
    if target.role.as_deref() == Some("player") {
        state
            .users
            .change_password_tenant(db, id, &dto.newPassword, &claims)
            .await?;
    } else {
        if !crate::access::has(&claims, "team:write") {
            return Err(crate::error::AppError::Forbidden(
                "Team write permission required".into(),
            ));
        }
        identities(&state)?
            .change_password(db.tenant_id(), id, &dto.newPassword)
            .await?;
    }
    ok(serde_json::json!({ "message": "Password changed successfully" }))
}

#[utoipa::path(
    post,
    path = "/users/{id}/totp/setup",
    params(
        ("id" = Uuid, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "TOTP setup initiated", body = crate::openapi::responses::TotpSetupEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn setup_totp(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<TotpSetupResponseDto> {
    let result = identities(&state)?
        .setup_totp(state.business_db(&claims).await?.tenant_id(), id)
        .await?;
    ok(result)
}

#[utoipa::path(
    post,
    path = "/users/{id}/totp/verify",
    params(
        ("id" = Uuid, Path, description = "User ID"),
    ),
    request_body = VerifyTotpSetupDto,
    responses(
        (status = 200, description = "TOTP enabled", body = crate::openapi::responses::TotpSetupEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn verify_totp_setup(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<VerifyTotpSetupDto>,
) -> ApiResult<TotpSetupResponseDto> {
    let result = identities(&state)?
        .verify_totp_setup(state.business_db(&claims).await?.tenant_id(), id, &dto.code)
        .await?;
    ok(result)
}

#[utoipa::path(
    delete,
    path = "/users/{id}/totp",
    params(
        ("id" = Uuid, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "TOTP disabled", body = serde_json::Value),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn disable_totp(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<serde_json::Value> {
    identities(&state)?
        .disable_totp(state.business_db(&claims).await?.tenant_id(), id)
        .await?;
    ok(serde_json::json!({ "disabled": true }))
}

#[allow(non_snake_case)]
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct UpdateAvatarDto {
    /// Public URL returned by `/uploads/presign` with `purpose: "avatar"`, or null to remove.
    pub avatarUrl: Option<String>,
}

/// Set or remove the signed-in panel user's own profile photo (DRAFT-0042).
#[utoipa::path(
    put,
    path = "/users/me/avatar",
    request_body = UpdateAvatarDto,
    responses(
        (status = 200, description = "Updated profile", body = crate::openapi::responses::PanelUserEnvelope),
        (status = 400, description = "URL is not an uploaded profile photo", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn update_own_avatar(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<UpdateAvatarDto>,
) -> ApiResult<crate::dto::AuthUserDto> {
    let id = claims
        .user_id_uuid()
        .ok_or_else(|| crate::error::AppError::Unauthorized("Invalid session".into()))?;
    let url = dto
        .avatarUrl
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty());
    if let Some(url) = url {
        if !state
            .storage
            .owns_public_url(url, &format!("avatars/{}", claims.userId))
        {
            return Err(crate::error::AppError::BadRequest(
                "avatarUrl must be a profile photo uploaded for this account".into(),
            ));
        }
    }
    let db = state.business_db(&claims).await?;
    identities(&state)?
        .set_avatar(db.tenant_id(), id, url)
        .await?;
    sync_identity(&state, db.clone()).await?;
    ok(state.users.get_by_id_tenant(db, id).await?.to_auth_user())
}

use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{ok, ApiResult};
use crate::middleware::{AdminOrStaff, AdminUser};
use crate::models::{TotpSetupResponseDto, UpdateUserDto, User, UserFilterDto, VerifyTotpSetupDto};
use crate::openapi::responses::{ErrorEnvelope, UserEnvelope, UserPaginationEnvelope};

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
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<UserFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<User>> {
    let result = state.users.list(filters).await?;
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
pub async fn get_user(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult<User> {
    let user = state.users.get_by_id(id).await?;
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
    let user = state.users.update(id, dto, claims.user_id_uuid()).await?;
    ok(user)
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
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<TotpSetupResponseDto> {
    let result = state.users.setup_totp(id).await?;
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
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<VerifyTotpSetupDto>,
) -> ApiResult<TotpSetupResponseDto> {
    let result = state.users.verify_totp_setup(id, &dto.code).await?;
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
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<serde_json::Value> {
    state.users.disable_totp(id).await?;
    ok(serde_json::json!({ "disabled": true }))
}

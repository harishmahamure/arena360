use axum::{extract::State, Json};
use std::sync::Arc;

use crate::app::AppState;
use crate::dto::{
    created, ok, ApiResult, AuthResponseDto, LoginDto, OtpPendingResponse, RegisterDto,
    RegisterResponseDto, VerifyOtpDto,
};
use crate::middleware::AdminOrStaff;
use crate::openapi::responses::{
    AuthResponseEnvelope, ErrorEnvelope, OtpPendingEnvelope, RegisterResponseEnvelope,
};

#[utoipa::path(
    post,
    path = "/auth/login/admin",
    request_body = LoginDto,
    responses(
        (status = 200, description = "OTP sent", body = OtpPendingEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    tag = "auth"
)]
pub async fn login_admin(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<LoginDto>,
) -> ApiResult<OtpPendingResponse> {
    let result = state.auth.login_admin(dto).await?;
    ok(result)
}

#[utoipa::path(
    post,
    path = "/auth/login/staff",
    request_body = LoginDto,
    responses(
        (status = 200, description = "Authenticated", body = AuthResponseEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    tag = "auth"
)]
pub async fn login_staff(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<LoginDto>,
) -> ApiResult<AuthResponseDto> {
    let result = state.auth.login_staff(dto).await?;
    ok(result)
}

#[utoipa::path(
    post,
    path = "/auth/verify-otp",
    request_body = VerifyOtpDto,
    responses(
        (status = 200, description = "Authenticated", body = AuthResponseEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    tag = "auth"
)]
pub async fn verify_otp(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<VerifyOtpDto>,
) -> ApiResult<AuthResponseDto> {
    let result = state.auth.verify_otp(dto).await?;
    ok(result)
}

#[utoipa::path(
    post,
    path = "/auth/register",
    request_body = RegisterDto,
    responses(
        (status = 201, description = "User registered", body = RegisterResponseEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub async fn register(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<RegisterDto>,
) -> ApiResult<RegisterResponseDto> {
    let result = state.users.register(dto, &claims).await?;
    created(result)
}

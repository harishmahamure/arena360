use axum::{extract::State, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{
    created, ok, ApiResult, AuthResponseDto, LoginDto, OtpPendingResponse, RegisterDto,
    RegisterResponseDto, VerifyOtpDto,
};
use crate::error::AppError;
use crate::middleware::AdminOrStaff;
use crate::models::ClockInDto;
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
    let mut result = state.auth.login_staff(dto).await?;
    let user_id: Uuid = result
        .user
        .id
        .parse()
        .map_err(|_| AppError::Internal("Invalid user ID".to_string()))?;

    let shift = match state
        .shifts
        .clock_in(
            user_id,
            ClockInDto {
                notes: Some("Auto-started on login".to_string()),
            },
            user_id,
        )
        .await
    {
        Ok(shift) => shift,
        Err(AppError::Conflict(_)) => state
            .shifts
            .get_active(user_id)
            .await?
            .ok_or_else(|| AppError::Internal("Active shift conflict".to_string()))?,
        Err(error) => return Err(error),
    };

    let _ = state
        .cash_registers
        .ensure_open_for_shift(shift.id, user_id, 0.0)
        .await;

    result.shiftId = Some(shift.id.to_string());
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

use axum::{extract::State, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{
    created, ok, ApiResult, AuthResponseDto, CreateSsoTokenDto, CreateSsoTokenResponseDto,
    DevicePairingDto, DevicePairingResponseDto, LoginDto, OtpPendingResponse, PlayerLoginDto,
    RedeemSsoTokenDto, RegisterDto, RegisterResponseDto, StaffLoginDto, VerifyOtpDto,
};
use crate::validation::{is_playstation_device_type, require_playstation_device_type};
use crate::error::AppError;
use crate::middleware::{AdminOrStaff, DeviceUser};
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
    request_body = StaffLoginDto,
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
    Json(dto): Json<StaffLoginDto>,
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
        .carry_forward_balance(user_id, shift.id, user_id)
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
    path = "/auth/login/player",
    request_body = PlayerLoginDto,
    responses(
        (status = 200, description = "Authenticated", body = AuthResponseEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden — device not registered, maintenance, or no usable plan (PLAN_EXPIRED, PLAN_EXHAUSTED, PLAN_NOT_ACTIVATED, TIME_WINDOW_VIOLATION, DEVICE_TYPE_NOT_ALLOWED)", body = ErrorEnvelope),
        (status = 409, description = "Conflict — PLAYER_ALREADY_IN_SESSION", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub async fn login_player(
    State(state): State<Arc<AppState>>,
    device_user: DeviceUser,
    Json(dto): Json<PlayerLoginDto>,
) -> ApiResult<AuthResponseDto> {
    let device_id = device_user.device_id()?;
    let device = state.devices.get_by_id(device_id).await?;

    if let Some(fingerprint) = &dto.fingerprint {
        state
            .devices
            .verify_fingerprint_drift(&device, fingerprint)
            .await?;
    }

    let result = state
        .auth
        .login_player(
            &device,
            LoginDto {
                username: dto.username,
                password: dto.password,
            },
        )
        .await?;
    ok(result)
}

#[utoipa::path(
    post,
    path = "/auth/sso/tokens",
    request_body = CreateSsoTokenDto,
    responses(
        (status = 200, description = "SSO token created"),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub async fn create_sso_token(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateSsoTokenDto>,
) -> ApiResult<CreateSsoTokenResponseDto> {
    let created_by = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user".to_string()))?;

    if let Some(device_id) = dto.deviceId.as_deref() {
        let device_uuid = Uuid::parse_str(device_id)
            .map_err(|_| AppError::BadRequest("Invalid deviceId".to_string()))?;
        let device = state.devices.get_by_id(device_uuid).await?;
        if !is_playstation_device_type(&device.device_type) {
            return Err(AppError::forbidden_code("DEVICE_TYPE_NOT_ALLOWED"));
        }
    }

    let created_token = state
        .auth
        .create_sso_token(dto.clone(), created_by)
        .await?;

    if let Some(device_id) = &created_token.deviceId {
        let channel = format!("device:{device_id}");
        let payload = serde_json::json!({
            "token": created_token.token,
            "expiresAt": created_token.expiresAt,
            "purpose": dto.purpose,
            "deviceId": device_id,
        });
        let _ = state
            .outbox
            .publish(&channel, "sso.token.created", payload, None, None, false)
            .await;
    }

    ok(created_token)
}

#[utoipa::path(
    post,
    path = "/auth/sso/redeem",
    request_body = RedeemSsoTokenDto,
    responses(
        (status = 200, description = "Staff JWT issued", body = AuthResponseEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    tag = "auth"
)]
pub async fn redeem_sso_token(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<RedeemSsoTokenDto>,
) -> ApiResult<AuthResponseDto> {
    let result = state.auth.redeem_sso_token(dto).await?;
    ok(result)
}

#[utoipa::path(
    post,
    path = "/auth/device-pairing",
    request_body = DevicePairingDto,
    responses(
        (status = 200, description = "Pairing JWT for pre-provision WS"),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    tag = "auth"
)]
pub async fn device_pairing(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<DevicePairingDto>,
) -> ApiResult<DevicePairingResponseDto> {
    let device_id = Uuid::parse_str(dto.deviceId.trim())
        .map_err(|_| AppError::BadRequest("Invalid deviceId".to_string()))?;
    let device = state.devices.get_by_id(device_id).await?;
    require_playstation_device_type(Some(device.device_type.clone()))?;
    let pairing = state.auth.generate_pairing_token(device_id)?;
    ok(pairing)
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

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::app::AppState;
use crate::dto::{
    created, ok, ApiResult, AuthResponseDto, KioskRegisterDto, KioskRegisterResponseDto, LoginDto,
    PanelLoginResponseDto, PanelMfaDto, PlayerLoginDto, RegisterDto, RegisterResponseDto,
    StaffLoginDto,
};
use crate::error::AppError;
use crate::middleware::{AdminOrStaff, AdminUser, DeviceUser};
use crate::openapi::responses::{
    AuthResponseEnvelope, ErrorEnvelope, KioskRegisterResponseEnvelope, PanelLoginResponseEnvelope,
    RegisterResponseEnvelope,
};
use crate::services::KioskRegistrationRateLimiter;

async fn local_auth(state: &AppState, token: &str) -> Result<AuthResponseDto, AppError> {
    let claims = crate::middleware::auth::decode_token(state, token)?;
    let user = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))?;
    if let Some(router) = &state.routing {
        let tenant = uuid::Uuid::parse_str(&claims.tenantId)
            .map_err(|_| AppError::Unauthorized("Invalid tenant identity".into()))?;
        if let Some(auth) = router.finish_auth(tenant, token, false).await? {
            let returned = crate::middleware::auth::decode_token(state, &auth.accessToken)?;
            if returned.tenantId != claims.tenantId || returned.userId != claims.userId {
                return Err(AppError::Forbidden(
                    "Owner authentication changed identity or tenant".into(),
                ));
            }
            return Ok(auth);
        }
    }
    state
        .auth
        .issue_tenant_auth_response(state.business_db(&claims).await?, user)
        .await
}
async fn local_panel(
    state: &AppState,
    response: PanelLoginResponseDto,
) -> Result<PanelLoginResponseDto, AppError> {
    match response {
        PanelLoginResponseDto::Authenticated { access_token, .. } => {
            let auth = local_auth(state, &access_token).await?;
            let claims = crate::middleware::auth::decode_token(state, &auth.accessToken)?;
            let next_step = if crate::access::has(&claims, "shifts:write")
                && !crate::access::has(&claims, "access:manage")
            {
                "shift_setup"
            } else {
                "dashboard"
            };
            Ok(PanelLoginResponseDto::Authenticated {
                access_token: auth.accessToken,
                user: auth.user,
                next_step: next_step.into(),
            })
        }
        challenge => Ok(challenge),
    }
}

#[utoipa::path(
    post,
    path = "/auth/login/panel",
    request_body = StaffLoginDto,
    responses(
        (status = 200, description = "Authenticated or MFA challenge issued", body = PanelLoginResponseEnvelope),
        (status = 401, description = "Invalid credentials", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    tag = "auth"
)]
pub async fn login_panel(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<StaffLoginDto>,
) -> ApiResult<PanelLoginResponseDto> {
    ok(local_panel(&state, state.auth.login_panel(dto).await?).await?)
}

#[utoipa::path(
    post,
    path = "/auth/login/panel/mfa",
    request_body = PanelMfaDto,
    responses(
        (status = 200, description = "MFA verified and authenticated", body = PanelLoginResponseEnvelope),
        (status = 401, description = "Invalid or expired MFA challenge", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    tag = "auth"
)]
pub async fn verify_panel_mfa(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<PanelMfaDto>,
) -> ApiResult<PanelLoginResponseDto> {
    ok(local_panel(&state, state.auth.verify_panel_mfa(dto).await?).await?)
}

#[utoipa::path(
    post,
    path = "/auth/login/admin",
    request_body = StaffLoginDto,
    responses(
        (status = 200, description = "Authenticated", body = AuthResponseEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    tag = "auth"
)]
pub async fn login_admin(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<StaffLoginDto>,
) -> ApiResult<AuthResponseDto> {
    let control = state.auth.login_admin(dto).await?;
    let result = local_auth(&state, &control.accessToken).await?;
    let claims = crate::middleware::auth::decode_token(&state, &result.accessToken)?;
    if let Some(router) = &state.routing {
        let tenant = uuid::Uuid::parse_str(&claims.tenantId)
            .map_err(|_| AppError::Unauthorized("Invalid tenant identity".into()))?;
        if let Some(auth) = router
            .finish_admin_login(tenant, &result.accessToken)
            .await?
        {
            let returned = crate::middleware::auth::decode_token(&state, &auth.accessToken)?;
            if returned.tenantId != claims.tenantId || returned.userId != claims.userId {
                return Err(AppError::Forbidden(
                    "Owner authentication changed identity or tenant".into(),
                ));
            }
            return ok(auth);
        }
    }
    close_local_admin_shift(&state, &claims).await?;
    ok(result)
}
async fn close_local_admin_shift(
    state: &AppState,
    claims: &crate::dto::JwtUserClaims,
) -> Result<(), AppError> {
    let user = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid identity".into()))?;
    let db = state.business_db(claims).await?;
    let repo = crate::repositories::TenantShiftRepository::new(db.clone());
    if let Some(active) = repo.find_active_by_user(user).await? {
        let location = repo.location_id(active.id).await?;
        crate::access::scope::LocationScope::resolve_tenant(
            db,
            claims,
            "shifts:force_close",
            Some(location),
        )
        .await?;
        repo.force_close(active.id, user).await?;
    }
    Ok(())
}
#[utoipa::path(post,path="/auth/admin-shift-close",responses((status=200,body=AuthResponseEnvelope),(status=403,body=ErrorEnvelope)),security(("bearer_auth"=[])),tag="auth")]
pub async fn complete_admin_login(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<AuthResponseDto> {
    let user = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid identity".into()))?;
    let auth = state
        .auth
        .issue_local_tenant_auth_response(
            state.business_db(&claims).await?,
            user,
            &claims.allowedTenants,
        )
        .await?;
    close_local_admin_shift(&state, &claims).await?;
    ok(auth)
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
    let control = state.auth.login_staff(dto).await?;
    let result = local_auth(&state, &control.accessToken).await?;
    let claims = crate::middleware::auth::decode_token(&state, &result.accessToken)?;
    if let Some(router) = &state.routing {
        let tenant = uuid::Uuid::parse_str(&claims.tenantId)
            .map_err(|_| AppError::Unauthorized("Invalid tenant identity".into()))?;
        if let Some(auth) = router
            .finish_auth(tenant, &result.accessToken, true)
            .await?
        {
            let returned = crate::middleware::auth::decode_token(&state, &auth.accessToken)?;
            if returned.tenantId != claims.tenantId || returned.userId != claims.userId {
                return Err(AppError::Forbidden(
                    "Owner authentication changed identity or tenant".into(),
                ));
            }
            return ok(auth);
        }
    }
    ok(finish_local_staff_login(&state, &claims).await?)
}

async fn finish_local_staff_login(
    state: &AppState,
    claims: &crate::dto::JwtUserClaims,
) -> Result<AuthResponseDto, AppError> {
    crate::middleware::auth::require_staff_for_counter(claims)?;
    let db = state.business_db(claims).await?;
    let user = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))?;
    let scope = crate::access::scope::LocationScope::resolve_tenant(
        db.clone(),
        claims,
        "shifts:write",
        None,
    )
    .await?;
    if scope.locations.len() != 1 {
        return Err(AppError::bad_request_code("LOCATION_REQUIRED", None));
    }
    let venue = scope.locations[0];
    // Issue the token before opening the register; a signing failure cannot leave
    // an unreported financial mutation behind.
    let mut auth = state
        .auth
        .issue_local_tenant_auth_response(db.clone(), user, &claims.allowedTenants)
        .await?;
    let opening = crate::repositories::TenantCashRegisterRepository::new(db.clone())
        .preview_carry_forward_balance_for(venue)
        .await?;
    let started = crate::repositories::TenantShiftRepository::new(db)
        .start_confirmed(
            user,
            crate::models::StartShiftDto {
                opening_balance: opening,
                opening_denominations: None,
                notes: None,
                venue_location_id: Some(venue),
            },
            user,
        )
        .await?;
    auth.shiftId = Some(started.shift.id.to_string());
    Ok(auth)
}
#[utoipa::path(post,path="/auth/staff-shift",responses((status=200,description="Resume the authenticated staff shift",body=AuthResponseEnvelope),(status=403,description="Shift permission required",body=ErrorEnvelope)),security(("bearer_auth"=[])),tag="auth")]
pub async fn resume_staff_shift(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
) -> ApiResult<AuthResponseDto> {
    ok(finish_local_staff_login(&state, &claims).await?)
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
    let device = state
        .devices
        .get_tenant(state.business_db(&device_user.0).await?, device_id)
        .await?;

    if let Some(fingerprint) = &dto.fingerprint {
        state
            .devices
            .verify_fingerprint_drift_tenant(
                state.business_db(&device_user.0).await?,
                &device,
                fingerprint,
            )
            .await?;
    }

    let result = state
        .auth
        .login_player_tenant(
            state.business_db(&device_user.0).await?,
            &device,
            LoginDto {
                username: dto.username,
                password: dto.password,
            },
            state.business_db(&device_user.0).await?.timezone().await?,
        )
        .await?;
    ok(result)
}

#[utoipa::path(
    post,
    path = "/auth/register/player",
    request_body = KioskRegisterDto,
    responses(
        (status = 201, description = "Player registered", body = KioskRegisterResponseEnvelope),
        (status = 400, description = "Bad request — validation (AUTH_WEAK_PASSWORD, field details)", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden — DEVICE_NOT_REGISTERED or DEVICE_UNDER_MAINTENANCE", body = ErrorEnvelope),
        (status = 409, description = "Conflict — AUTH_USERNAME_ALREADY_EXISTS", body = ErrorEnvelope),
        (status = 429, description = "Too many requests — REGISTRATION_RATE_LIMITED", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub async fn register_player(
    State(state): State<Arc<AppState>>,
    device_user: DeviceUser,
    Json(dto): Json<KioskRegisterDto>,
) -> ApiResult<KioskRegisterResponseDto> {
    let device_id = device_user.device_id()?;
    let device = state
        .devices
        .get_tenant(state.business_db(&device_user.0).await?, device_id)
        .await?;

    if device.registration_status != "registered" {
        return Err(AppError::forbidden_code("DEVICE_NOT_REGISTERED"));
    }
    if device.status == "under_maintenance" {
        return Err(AppError::forbidden_code("DEVICE_UNDER_MAINTENANCE"));
    }

    KioskRegistrationRateLimiter::new(state.cache.clone())
        .check_and_record(device_id)
        .await?;

    let result = state
        .users
        .register_from_kiosk_tenant(state.business_db(&device_user.0).await?, dto)
        .await?;
    created(result)
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
    let result = state
        .users
        .register_tenant(state.business_db(&claims).await?, dto, &claims)
        .await?;
    created(result)
}

/// Return only the authenticated panel user's public profile. The auth middleware
/// checks current account status, role, membership, and token expiry first.
#[utoipa::path(
    get, path = "/auth/me",
    responses(
        (status = 200, description = "Current panel account", body = crate::openapi::responses::PanelUserEnvelope),
        (status = 401, description = "Session expired or account access revoked", body = ErrorEnvelope),
        (status = 403, description = "Panel role required", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])), tag = "auth"
)]
pub async fn current_panel_user(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
) -> ApiResult<crate::dto::AuthUserDto> {
    let id = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid session".into()))?;
    ok(
        crate::repositories::TenantUserRepository::new(state.business_db(&claims).await?)
            .require_active_staff(id)
            .await?
            .to_auth_user(),
    )
}

/// Re-issue a panel access token for an active session so working staff are not
/// signed out mid-task. Idle timeout is enforced by the client; revoked access fails in middleware.
#[utoipa::path(
    post, path = "/auth/refresh",
    responses(
        (status = 200, description = "Renewed panel token", body = AuthResponseEnvelope),
        (status = 401, description = "Session expired or account access revoked", body = ErrorEnvelope),
        (status = 403, description = "Panel role required", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])), tag = "auth"
)]
pub async fn refresh_panel_session(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
) -> ApiResult<AuthResponseDto> {
    let id = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid session".into()))?;
    ok(state
        .auth
        .issue_local_tenant_auth_response(
            state.business_db(&claims).await?,
            id,
            &claims.allowedTenants,
        )
        .await?)
}

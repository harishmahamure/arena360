use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{
    created, kiosk_session_response, ok, ApiResult, EndKioskSessionDto, KioskSessionResponseDto,
    StartKioskSessionDto,
};
use crate::error::AppError;
use crate::middleware::PlayerUser;
use crate::models::EndSessionDto;
use crate::openapi::responses::ErrorEnvelope;

/// Start (or resume) a kiosk session for the authenticated player on the
/// authenticated device. Enforces the global single-session rule (ADR-0017).
#[utoipa::path(
    post,
    path = "/kiosk/sessions",
    request_body = StartKioskSessionDto,
    responses(
        (status = 201, description = "Session started or resumed"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden — no usable balance", body = ErrorEnvelope),
        (status = 409, description = "Conflict — PLAYER_ALREADY_IN_SESSION", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "kiosk"
)]
pub async fn start_session(
    player: PlayerUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<StartKioskSessionDto>,
) -> ApiResult<KioskSessionResponseDto> {
    let player_id = player.player_id()?;
    let device_id = player.device_id()?;
    let device = state
        .devices
        .get_tenant(state.business_db(&player.0).await?, device_id)
        .await?;

    if device.registration_status != "registered" {
        return Err(AppError::forbidden_code("DEVICE_NOT_REGISTERED"));
    }
    if device.status == "under_maintenance" {
        return Err(AppError::forbidden_code("DEVICE_UNDER_MAINTENANCE"));
    }

    let balance_id = match dto.balanceId.as_deref() {
        Some(raw) => Some(
            Uuid::parse_str(raw)
                .map_err(|_| AppError::BadRequest("Invalid balanceId".to_string()))?,
        ),
        None => None,
    };

    let started = state
        .sessions
        .start_for_player_tenant(
            state.business_db(&player.0).await?,
            player_id,
            &device,
            balance_id,
        )
        .await?;

    created(kiosk_session_response(&started, None))
}

/// The authenticated player's current open session, or `null` when none.
/// Polled by the kiosk HUD to resync the countdown and detect remote ends.
#[utoipa::path(
    get,
    path = "/kiosk/sessions/current",
    responses(
        (status = 200, description = "Current session or null"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "kiosk"
)]
pub async fn current_session(
    player: PlayerUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Option<KioskSessionResponseDto>> {
    let player_id = player.player_id()?;
    let open = state
        .sessions
        .open_kiosk_session_for_player_tenant(state.business_db(&player.0).await?, player_id)
        .await?;
    ok(open.map(|s| kiosk_session_response(&s, None)))
}

/// Heartbeat the authenticated player's current kiosk session. This deducts
/// newly elapsed usage server-side and returns authoritative remaining time.
#[utoipa::path(
    patch,
    path = "/kiosk/sessions/{id}/heartbeat",
    params(("id" = Uuid, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Session heartbeat accepted"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden — not the player's session", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "kiosk"
)]
pub async fn heartbeat_session(
    player: PlayerUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<KioskSessionResponseDto> {
    let player_id = player.player_id()?;
    let device_id = player.device_id()?;
    let heartbeat = state
        .sessions
        .heartbeat_for_player_tenant(
            state.business_db(&player.0).await?,
            id,
            player_id,
            device_id,
        )
        .await?;

    ok(kiosk_session_response(&heartbeat, None))
}

/// End the player's own session. The session's balance must belong to the
/// authenticated player.
#[utoipa::path(
    patch,
    path = "/kiosk/sessions/{id}/end",
    operation_id = "kiosk_end_session",
    params(("id" = Uuid, Path, description = "Session ID")),
    request_body = EndKioskSessionDto,
    responses(
        (status = 200, description = "Session ended"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden — not the player's session", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "kiosk"
)]
pub async fn end_session(
    player: PlayerUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<EndKioskSessionDto>,
) -> ApiResult<KioskSessionResponseDto> {
    let player_id = player.player_id()?;

    let db = state.business_db(&player.0).await?;
    let timezone = db.timezone().await?;
    let session = state
        .sessions
        .get_by_id_tenant(db.clone(), id, timezone.clone())
        .await?;
    if session.device_id != player.device_id()? {
        return Err(AppError::Forbidden(
            "Session belongs to another device".into(),
        ));
    }
    let owner = session
        .balance
        .as_ref()
        .map(|b| b.player_id)
        .ok_or_else(|| AppError::Forbidden("Session has no owning player".to_string()))?;
    if owner != player_id {
        return Err(AppError::Forbidden(
            "Cannot end another player's session".to_string(),
        ));
    }

    // Idempotent end (D18): a replayed offline end-intent for an
    // already-closed session is a no-op, never a second deduction.
    if session.end_time.is_some() {
        let balance_id = session.balance_id;
        let balance = state
            .balances
            .get_raw_tenant(db.clone(), balance_id)
            .await?;
        let remaining = balance.remaining_minutes;
        let deduction_profile = session
            .balance
            .as_ref()
            .and_then(|b| b.deduction_profile.as_ref())
            .and_then(|p| serde_json::to_value(p).ok());
        let time_credits_consumed = session.time_credits_consumed.map(|v| v as f64);
        let expiry_date = crate::time::utc_timestamp(&balance.expiry_date);
        return ok(KioskSessionResponseDto {
            sessionId: session.id.to_string(),
            balanceId: balance_id.to_string(),
            deviceId: session.device_id.to_string(),
            startTime: crate::time::utc_timestamp(&session.start_time),
            remainingMinutes: remaining as f64,
            walletBalanceMinutes: remaining as f64,
            resumed: false,
            endTime: session.end_time.as_ref().map(crate::time::utc_timestamp),
            deductionProfile: deduction_profile.and_then(|value| {
                serde_json::from_value::<crate::models::deduction_profile::DeductionProfile>(value)
                    .ok()
            }),
            cafeTimezone: session.cafe_timezone.clone(),
            timeCreditsConsumed: time_credits_consumed,
            expiryDate: expiry_date,
        });
    }

    let ended = state
        .sessions
        .end_tenant(
            db.clone(),
            id,
            EndSessionDto {
                end_time: None,
                time_credits_consumed: None,
                staff_totp: None,
                reason: Some(dto.reason.unwrap_or_else(|| "voluntary".to_string())),
            },
            None,
        )
        .await?;

    let balance_id = ended.balance_id;
    let balance = state
        .balances
        .get_raw_tenant(db.clone(), balance_id)
        .await?;
    let remaining = balance.remaining_minutes;
    let deduction_profile =
        crate::services::session_service::session_profile_value(&balance, &ended).cloned();
    let time_credits_consumed = ended.time_credits_consumed.map(|v| v as f64);
    let expiry_date = crate::time::utc_timestamp(&balance.expiry_date);

    ok(KioskSessionResponseDto {
        sessionId: ended.id.to_string(),
        balanceId: balance_id.to_string(),
        deviceId: ended.device_id.to_string(),
        startTime: crate::time::utc_timestamp(&ended.start_time),
        remainingMinutes: remaining as f64,
        walletBalanceMinutes: remaining as f64,
        resumed: false,
        endTime: ended.end_time.as_ref().map(crate::time::utc_timestamp),
        deductionProfile: deduction_profile.and_then(|value| {
            serde_json::from_value::<crate::models::deduction_profile::DeductionProfile>(value).ok()
        }),
        cafeTimezone: session.cafe_timezone.clone(),
        timeCreditsConsumed: time_credits_consumed,
        expiryDate: expiry_date,
    })
}

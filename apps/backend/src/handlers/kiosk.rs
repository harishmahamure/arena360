use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{
    created, ok, ApiResult, EndKioskSessionDto, KioskSessionResponseDto, StartKioskSessionDto,
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
    let device = state.devices.get_by_id(device_id).await?;

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
        .start_for_player(player_id, &device, balance_id)
        .await?;

    created(KioskSessionResponseDto {
        sessionId: started.session.id.to_string(),
        balanceId: started.balance_id.to_string(),
        deviceId: device.id.to_string(),
        startTime: started.session.start_time.to_rfc3339(),
        remainingMinutes: started.remaining_minutes as f64,
        resumed: started.resumed,
        endTime: None,
    })
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
    let open = state.sessions.open_session_for_player(player_id).await?;
    ok(open.map(|s| KioskSessionResponseDto {
        sessionId: s.session_id.to_string(),
        balanceId: s.balance_id.to_string(),
        deviceId: s.device_id.to_string(),
        startTime: s.start_time.to_rfc3339(),
        remainingMinutes: s.remaining_minutes as f64,
        resumed: true,
        endTime: None,
    }))
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
        .heartbeat_for_player(id, player_id, device_id)
        .await?;

    ok(KioskSessionResponseDto {
        sessionId: heartbeat.session.id.to_string(),
        balanceId: heartbeat.balance_id.to_string(),
        deviceId: heartbeat.session.device_id.to_string(),
        startTime: heartbeat.session.start_time.to_rfc3339(),
        remainingMinutes: heartbeat.remaining_minutes as f64,
        resumed: true,
        endTime: None,
    })
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

    let session = state.sessions.get_by_id(id).await?;
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
        let remaining = match session.balance_id {
            Some(balance_id) => state.balances.get_raw(balance_id).await?.remaining_minutes,
            None => 0,
        };
        return ok(KioskSessionResponseDto {
            sessionId: session.id.to_string(),
            balanceId: session
                .balance_id
                .map(|b| b.to_string())
                .unwrap_or_default(),
            deviceId: session.device_id.to_string(),
            startTime: session.start_time.to_rfc3339(),
            remainingMinutes: remaining as f64,
            resumed: false,
            endTime: session.end_time.map(|t| t.to_rfc3339()),
        });
    }

    let ended = state
        .sessions
        .end(
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

    let remaining = match ended.balance_id {
        Some(balance_id) => state.balances.get_raw(balance_id).await?.remaining_minutes,
        None => 0,
    };

    ok(KioskSessionResponseDto {
        sessionId: ended.id.to_string(),
        balanceId: ended.balance_id.map(|b| b.to_string()).unwrap_or_default(),
        deviceId: ended.device_id.to_string(),
        startTime: ended.start_time.to_rfc3339(),
        remainingMinutes: remaining as f64,
        resumed: false,
        endTime: ended.end_time.map(|t| t.to_rfc3339()),
    })
}

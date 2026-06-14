use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{ok, ApiResult, EndTvSessionDto, TvSessionResponseDto};
use crate::middleware::DeviceUser;
use crate::openapi::responses::ErrorEnvelope;

#[utoipa::path(
    get,
    path = "/tv/sessions/current",
    responses(
        (status = 200, description = "Current TV session or null"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden — not a PlayStation device", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "console-tv"
)]
pub async fn current_session(
    device: DeviceUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Option<TvSessionResponseDto>> {
    let device_id = device.device_id()?;
    let station = state.devices.get_by_id(device_id).await?;
    let current = state
        .sessions
        .open_tv_session_for_device(&station)
        .await?;
    ok(current)
}

#[utoipa::path(
    patch,
    path = "/tv/sessions/{id}/end",
    params(("id" = Uuid, Path, description = "Session ID")),
    request_body = EndTvSessionDto,
    responses(
        (status = 200, description = "Session ended"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "console-tv"
)]
pub async fn end_session(
    device: DeviceUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<EndTvSessionDto>,
) -> ApiResult<TvSessionResponseDto> {
    let device_id = device.device_id()?;
    let station = state.devices.get_by_id(device_id).await?;
    let reason = dto.reason.or(Some("auto".to_string()));
    let ended = state
        .sessions
        .end_tv_session_for_device(&station, id, reason)
        .await?;
    ok(ended)
}

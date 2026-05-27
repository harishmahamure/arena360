use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::middleware::AdminUser;
use crate::models::{CreateDeviceGameDto, DeviceGameFilterDto, DeviceGameResponse, UpdateDeviceGameDto};
use crate::openapi::responses::{
    DeviceGameEnvelope, DeviceGamePaginationEnvelope, ErrorEnvelope,
};

#[utoipa::path(
    get,
    path = "/device-games",
    responses(
        (status = 200, description = "List device games", body = DeviceGamePaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "device-games"
)]
pub async fn list_device_games(
    State(state): State<Arc<AppState>>,
    Query(filters): Query<DeviceGameFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<DeviceGameResponse>> {
    let result = state.device_games.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/device-games/device/{device_id}",
    params(
        ("device_id" = Uuid, Path, description = "Device ID"),
    ),
    responses(
        (status = 200, description = "List device games by device", body = DeviceGamePaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "device-games"
)]
pub async fn list_device_games_by_device(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<Uuid>,
    Query(filters): Query<DeviceGameFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<DeviceGameResponse>> {
    let result = state.device_games.list_by_device(device_id, filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/device-games/game/{game_id}",
    params(
        ("game_id" = Uuid, Path, description = "Game ID"),
    ),
    responses(
        (status = 200, description = "List device games by game", body = DeviceGamePaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "device-games"
)]
pub async fn list_device_games_by_game(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<Uuid>,
    Query(filters): Query<DeviceGameFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<DeviceGameResponse>> {
    let result = state.device_games.list_by_game(game_id, filters).await?;
    ok(result)
}

#[utoipa::path(
    post,
    path = "/device-games",
    request_body = CreateDeviceGameDto,
    responses(
        (status = 201, description = "Create device game", body = DeviceGameEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "device-games"
)]
pub async fn create_device_game(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateDeviceGameDto>,
) -> ApiResult<DeviceGameResponse> {
    let device_game = state.device_games.create(dto).await?;
    created(device_game)
}

#[utoipa::path(
    get,
    path = "/device-games/{id}",
    params(
        ("id" = Uuid, Path, description = "Device game ID"),
    ),
    responses(
        (status = 200, description = "Get device game", body = DeviceGameEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "device-games"
)]
pub async fn get_device_game(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<DeviceGameResponse> {
    let device_game = state.device_games.get_by_id(id).await?;
    ok(device_game)
}

#[utoipa::path(
    patch,
    path = "/device-games/{id}",
    params(
        ("id" = Uuid, Path, description = "Device game ID"),
    ),
    request_body = UpdateDeviceGameDto,
    responses(
        (status = 200, description = "Update device game", body = DeviceGameEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "device-games"
)]
pub async fn update_device_game(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateDeviceGameDto>,
) -> ApiResult<DeviceGameResponse> {
    let device_game = state.device_games.update(id, dto).await?;
    ok(device_game)
}

#[utoipa::path(
    delete,
    path = "/device-games/{id}",
    params(
        ("id" = Uuid, Path, description = "Device game ID"),
    ),
    responses(
        (status = 204, description = "Soft-deactivated (isActive=false)"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "device-games"
)]
pub async fn delete_device_game(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, crate::error::AppError> {
    state.device_games.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

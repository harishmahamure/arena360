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
use crate::models::{
    CreateDeviceDto, Device, DeviceFilterDto, UpdateDeviceDto, UpdateDeviceStatusDto,
};
use crate::openapi::responses::{
    DeviceEnvelope, DevicePaginationEnvelope, ErrorEnvelope,
};

#[utoipa::path(
    get,
    path = "/devices",
    responses(
        (status = 200, description = "List devices", body = DevicePaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "devices"
)]
pub async fn list_devices(
    State(state): State<Arc<AppState>>,
    Query(filters): Query<DeviceFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<Device>> {
    let result = state.devices.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/devices/{id}",
    params(
        ("id" = Uuid, Path, description = "Device ID"),
    ),
    responses(
        (status = 200, description = "Get device", body = DeviceEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "devices"
)]
pub async fn get_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Device> {
    let device = state.devices.get_by_id(id).await?;
    ok(device)
}

#[utoipa::path(
    post,
    path = "/devices",
    request_body = CreateDeviceDto,
    responses(
        (status = 201, description = "Create device", body = DeviceEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "devices"
)]
pub async fn create_device(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateDeviceDto>,
) -> ApiResult<Device> {
    let device = state.devices.create(dto).await?;
    created(device)
}

#[utoipa::path(
    patch,
    path = "/devices/{id}",
    params(
        ("id" = Uuid, Path, description = "Device ID"),
    ),
    request_body = UpdateDeviceDto,
    responses(
        (status = 200, description = "Update device", body = DeviceEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "devices"
)]
pub async fn update_device(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateDeviceDto>,
) -> ApiResult<Device> {
    let device = state.devices.update(id, dto).await?;
    ok(device)
}

#[utoipa::path(
    patch,
    path = "/devices/{id}/status",
    params(
        ("id" = Uuid, Path, description = "Device ID"),
    ),
    request_body = UpdateDeviceStatusDto,
    responses(
        (status = 200, description = "Update device status", body = DeviceEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "devices"
)]
pub async fn update_device_status(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateDeviceStatusDto>,
) -> ApiResult<Device> {
    let device = state.devices.update_status(id, dto).await?;
    ok(device)
}

#[utoipa::path(
    delete,
    path = "/devices/{id}",
    params(
        ("id" = Uuid, Path, description = "Device ID"),
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "devices"
)]
pub async fn delete_device(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, crate::error::AppError> {
    state.devices.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

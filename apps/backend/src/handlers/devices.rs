use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::error::AppError;
use crate::dto::{
    created, ok, ApiResult, DeviceRegisterResponseDto, ProvisionDeviceDto, RegisteredDeviceDto,
};
use crate::middleware::{AdminOrStaff, AdminUser};
use crate::validation::{is_playstation_device_type, require_playstation_device_type};
use crate::models::{
    normalize_device_type, CreateDeviceDto, Device, DeviceFilterDto, UpdateDeviceDto,
    UpdateDeviceStatusDto,
};
use crate::openapi::responses::{DeviceEnvelope, DevicePaginationEnvelope, ErrorEnvelope};

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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateDeviceDto>,
) -> ApiResult<Device> {
    let device = state.devices.create(dto, claims.user_id_uuid()).await?;
    created(device)
}

#[utoipa::path(
    post,
    path = "/devices/provision",
    request_body = ProvisionDeviceDto,
    responses(
        (status = 200, description = "Device provisioned; returns device token"),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 409, description = "Device name already exists", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "devices"
)]
pub async fn provision_device(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(mut dto): Json<ProvisionDeviceDto>,
) -> ApiResult<DeviceRegisterResponseDto> {
    if dto.provisionClient.as_deref() == Some("console-tv") {
        dto.deviceType = Some(require_playstation_device_type(dto.deviceType)?);
    } else if let Some(ref device_type) = dto.deviceType {
        if normalize_device_type(device_type)
            .is_some_and(|normalized| is_playstation_device_type(&normalized))
        {
            return Err(AppError::forbidden_code("DEVICE_TYPE_NOT_ALLOWED"));
        }
    }

    let device = state.devices.provision(dto, claims.user_id_uuid()).await?;
    let token = state.auth.generate_device_token(device.id)?;
    ok(DeviceRegisterResponseDto {
        accessToken: token,
        device: RegisteredDeviceDto::from(device),
    })
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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateDeviceDto>,
) -> ApiResult<Device> {
    let device = state.devices.update(id, dto, claims.user_id_uuid()).await?;
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
        (status = 204, description = "Soft-deactivated (deletedAt set)"),
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

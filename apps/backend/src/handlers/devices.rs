use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{
    created, ok, ApiResult, DeviceRegisterResponseDto, ProvisionDeviceDto, RegisteredDeviceDto,
};
use crate::error::AppError;
use crate::middleware::{AdminOrStaff, AdminUser, AuthUser};
use crate::models::{
    normalize_device_type, CreateDeviceDto, Device, DeviceFilterDto, UpdateDeviceDto,
    UpdateDeviceStatusDto,
};
use crate::openapi::responses::{DeviceEnvelope, DevicePaginationEnvelope, ErrorEnvelope};
use crate::validation::is_playstation_device_type;

fn organization_id(claims: &crate::dto::JwtUserClaims) -> Result<Uuid, AppError> {
    Uuid::parse_str(&claims.tenantId)
        .map_err(|_| AppError::Forbidden("Select an organization".into()))
}
fn actor_id(claims: &crate::dto::JwtUserClaims) -> Result<Uuid, AppError> {
    claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))
}

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
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Query(mut filters): Query<DeviceFilterDto>,
    headers: axum::http::HeaderMap,
) -> ApiResult<crate::dto::PaginationResult<Device>> {
    filters.location_id = filters
        .location_id
        .or(crate::access::scope::requested_location(&headers)?);
    let result = state
        .devices
        .list(
            filters,
            organization_id(&claims)?,
            actor_id(&claims)?,
            claims.is_admin(),
        )
        .await?;
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
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Device> {
    let device = state.devices.get_by_id(id).await?;
    if device.organization_id != organization_id(&claims)? {
        return Err(AppError::NotFound("Device not found".into()));
    }
    state
        .config
        .ensure_location_permission(
            device.organization_id,
            device.location_id,
            actor_id(&claims)?,
            "devices:read",
        )
        .await?;
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
    state
        .config
        .ensure_location_permission(
            organization_id(&claims)?,
            dto.location_id
                .unwrap_or(crate::models::DEFAULT_VENUE_LOCATION_ID),
            actor_id(&claims)?,
            "devices:write",
        )
        .await?;
    let device = state
        .devices
        .create(dto, claims.user_id_uuid(), organization_id(&claims)?)
        .await?;
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
    Json(dto): Json<ProvisionDeviceDto>,
) -> ApiResult<DeviceRegisterResponseDto> {
    state
        .config
        .ensure_location_permission(
            organization_id(&claims)?,
            dto.locationId
                .unwrap_or(crate::models::DEFAULT_VENUE_LOCATION_ID),
            actor_id(&claims)?,
            "devices:write",
        )
        .await?;
    if let Some(ref device_type) = dto.deviceType {
        if normalize_device_type(device_type)
            .is_some_and(|normalized| is_playstation_device_type(&normalized))
        {
            return Err(AppError::forbidden_code("DEVICE_TYPE_NOT_ALLOWED"));
        }
    }

    let device = state
        .devices
        .provision(dto, claims.user_id_uuid(), organization_id(&claims)?)
        .await?;
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
    let existing = state.devices.get_by_id(id).await?;
    if existing.organization_id != organization_id(&claims)? {
        return Err(AppError::NotFound("Device not found".into()));
    }
    state
        .config
        .ensure_location_permission(
            existing.organization_id,
            existing.location_id,
            actor_id(&claims)?,
            "devices:write",
        )
        .await?;
    if let Some(location_id) = dto.location_id {
        state
            .config
            .ensure_location_permission(
                existing.organization_id,
                location_id,
                actor_id(&claims)?,
                "devices:write",
            )
            .await?;
        if location_id != existing.location_id {
            let in_use: bool = sqlx::query_scalar(r#"SELECT EXISTS(
                SELECT 1 FROM usage_sessions WHERE "deviceId"=$1 AND "endTime" IS NULL AND "deletedAt" IS NULL
                UNION ALL
                SELECT 1 FROM kiosk_orders WHERE "deviceId"=$1 AND status IN ('pending','preparing')
            )"#).bind(id).fetch_one(&state.db).await?;
            if in_use {
                return Err(AppError::Conflict(
                    "Finish active sessions and kiosk orders before moving this device".into(),
                ));
            }
        }
    }
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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateDeviceStatusDto>,
) -> ApiResult<Device> {
    let existing = state.devices.get_by_id(id).await?;
    if existing.organization_id != organization_id(&claims)? {
        return Err(AppError::NotFound("Device not found".into()));
    }
    state
        .config
        .ensure_location_permission(
            existing.organization_id,
            existing.location_id,
            actor_id(&claims)?,
            "devices:write",
        )
        .await?;
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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, crate::error::AppError> {
    let existing = state.devices.get_by_id(id).await?;
    if existing.organization_id != organization_id(&claims)? {
        return Err(AppError::NotFound("Device not found".into()));
    }
    state
        .config
        .ensure_location_permission(
            existing.organization_id,
            existing.location_id,
            actor_id(&claims)?,
            "devices:write",
        )
        .await?;
    state.devices.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

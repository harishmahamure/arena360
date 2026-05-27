use axum::extract::{Path, Query, State};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult, PaginationResult};
use crate::middleware::{AdminOrStaff, AdminUser};
use crate::models::{ClockInDto, ClockOutDto, Shift, ShiftFilterDto};
use crate::openapi::responses::{ErrorEnvelope, ShiftEnvelope, ShiftPaginationEnvelope};

#[utoipa::path(
    post,
    path = "/shifts/clock-in",
    request_body = ClockInDto,
    responses(
        (status = 201, description = "Shift started", body = ShiftEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 409, description = "Already clocked in", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn clock_in(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<ClockInDto>,
) -> ApiResult<Shift> {
    let user_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID in token".to_string()))?;
    let shift = state.shifts.clock_in(user_id, dto, user_id).await?;
    created(shift)
}

#[utoipa::path(
    patch,
    path = "/shifts/clock-out",
    request_body = ClockOutDto,
    responses(
        (status = 200, description = "Shift ended", body = ShiftEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "No active shift", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn clock_out(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<ClockOutDto>,
) -> ApiResult<Shift> {
    let user_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID in token".to_string()))?;
    let shift = state.shifts.clock_out(user_id, dto, user_id).await?;
    ok(shift)
}

#[utoipa::path(
    get,
    path = "/shifts/active",
    responses(
        (status = 200, description = "Active shift or null", body = ShiftEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn get_active_shift(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Option<Shift>> {
    let user_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID in token".to_string()))?;
    let shift = state.shifts.get_active(user_id).await?;
    ok(shift)
}

#[utoipa::path(
    get,
    path = "/shifts",
    params(ShiftFilterDto),
    responses(
        (status = 200, description = "List shifts", body = ShiftPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn list_shifts(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(mut filters): Query<ShiftFilterDto>,
) -> ApiResult<PaginationResult<Shift>> {
    if !claims.is_admin() {
        let user_id: Uuid = claims.userId.parse().map_err(|_| {
            crate::error::AppError::BadRequest("Invalid user ID in token".to_string())
        })?;
        filters.user_id = Some(user_id);
    }
    let result = state.shifts.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/shifts/{id}",
    params(
        ("id" = Uuid, Path, description = "Shift ID"),
    ),
    responses(
        (status = 200, description = "Get shift", body = ShiftEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn get_shift(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Shift> {
    let shift = state.shifts.get_by_id(id).await?;

    if !claims.is_admin() {
        let user_id: Uuid = claims.userId.parse().map_err(|_| {
            crate::error::AppError::BadRequest("Invalid user ID in token".to_string())
        })?;
        if shift.user_id != user_id {
            return Err(crate::error::AppError::Forbidden(
                "Cannot view another user's shift".to_string(),
            ));
        }
    }

    ok(shift)
}

#[utoipa::path(
    patch,
    path = "/shifts/{id}/force-close",
    params(
        ("id" = Uuid, Path, description = "Shift ID"),
    ),
    responses(
        (status = 200, description = "Shift force-closed", body = ShiftEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn force_close_shift(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Shift> {
    let actor_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID in token".to_string()))?;
    let shift = state.shifts.force_close(id, actor_id).await?;
    ok(shift)
}

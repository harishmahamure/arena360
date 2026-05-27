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
use crate::models::{CreateUnitDto, Unit, UnitFilterDto, UpdateUnitDto};
use crate::openapi::responses::{ErrorEnvelope, UnitEnvelope, UnitPaginationEnvelope};

#[utoipa::path(
    get,
    path = "/units",
    responses(
        (status = 200, description = "List units", body = UnitPaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "units"
)]
pub async fn list_units(
    State(state): State<Arc<AppState>>,
    Query(filters): Query<UnitFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<Unit>> {
    let result = state.units.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/units/{id}",
    params(
        ("id" = Uuid, Path, description = "Unit ID"),
    ),
    responses(
        (status = 200, description = "Get unit", body = UnitEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "units"
)]
pub async fn get_unit(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult<Unit> {
    let unit = state.units.get_by_id(id).await?;
    ok(unit)
}

#[utoipa::path(
    post,
    path = "/units",
    request_body = CreateUnitDto,
    responses(
        (status = 201, description = "Create unit", body = UnitEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "units"
)]
pub async fn create_unit(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateUnitDto>,
) -> ApiResult<Unit> {
    let unit = state.units.create(dto, claims.user_id_uuid()).await?;
    created(unit)
}

#[utoipa::path(
    put,
    path = "/units/{id}",
    params(
        ("id" = Uuid, Path, description = "Unit ID"),
    ),
    request_body = UpdateUnitDto,
    responses(
        (status = 200, description = "Update unit", body = UnitEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "units"
)]
pub async fn update_unit(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateUnitDto>,
) -> ApiResult<Unit> {
    let unit = state.units.update(id, dto, claims.user_id_uuid()).await?;
    ok(unit)
}

#[utoipa::path(
    delete,
    path = "/units/{id}",
    params(
        ("id" = Uuid, Path, description = "Unit ID"),
    ),
    responses(
        (status = 204, description = "Soft-deactivated (isActive=false)"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "units"
)]
pub async fn delete_unit(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, crate::error::AppError> {
    state.units.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

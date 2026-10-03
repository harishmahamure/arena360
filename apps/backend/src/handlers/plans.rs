use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::middleware::{AdminUser, AuthUser};
use crate::error::AppError;
use crate::models::{CreatePlanDto, Plan, PlanFilterDto, UpdatePlanDto};
use crate::openapi::responses::{
    ActivePlansEnvelope, ErrorEnvelope, PlanEnvelope, PlanPaginationEnvelope,
};

#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlanLocationQuery { pub location_id: Option<Uuid> }

async fn pricing_location(state: &AppState, claims: &crate::dto::JwtUserClaims, requested: Option<Uuid>) -> Result<Uuid, AppError> {
    if claims.is_admin_or_staff() {
        let org = Uuid::parse_str(&claims.tenantId).map_err(|_| AppError::Forbidden("Select an organization".into()))?;
        let user = claims.user_id_uuid().ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))?;
        let location = requested.unwrap_or(crate::models::DEFAULT_VENUE_LOCATION_ID);
        state.config.ensure_location_permission(org, location, user, "plans:read").await?;
        Ok(location)
    } else if let Some(device_id) = claims.deviceId.as_deref() {
        let device_id = Uuid::parse_str(device_id).map_err(|_| AppError::Unauthorized("Invalid device identity".into()))?;
        Ok(state.devices.get_by_id(device_id).await?.location_id)
    } else {
        Ok(crate::models::DEFAULT_VENUE_LOCATION_ID)
    }
}

#[utoipa::path(
    get,
    path = "/plans",
    responses(
        (status = 200, description = "List plans", body = PlanPaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "plans"
)]
pub async fn list_plans(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Query(mut filters): Query<PlanFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<Plan>> {
    filters.location_id = Some(pricing_location(&state, &claims, filters.location_id).await?);
    let result = state.plans.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/plans/active",
    responses(
        (status = 200, description = "List active plans", body = ActivePlansEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "plans"
)]
pub async fn get_active_plans(AuthUser(claims): AuthUser, State(state): State<Arc<AppState>>, Query(query): Query<PlanLocationQuery>) -> ApiResult<Vec<Plan>> {
    let location = pricing_location(&state, &claims, query.location_id).await?;
    let plans = state.plans.get_active_for(Some(location)).await?;
    ok(plans)
}

#[utoipa::path(
    get,
    path = "/plans/{id}",
    params(
        ("id" = Uuid, Path, description = "Plan ID"),
    ),
    responses(
        (status = 200, description = "Get plan", body = PlanEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "plans"
)]
pub async fn get_plan(AuthUser(claims): AuthUser, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Query(query): Query<PlanLocationQuery>) -> ApiResult<Plan> {
    let location = pricing_location(&state, &claims, query.location_id).await?;
    let plan = state.plans.get_by_id_for(id, Some(location)).await?;
    ok(plan)
}

#[utoipa::path(
    post,
    path = "/plans",
    request_body = CreatePlanDto,
    responses(
        (status = 201, description = "Create plan", body = PlanEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "plans"
)]
pub async fn create_plan(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreatePlanDto>,
) -> ApiResult<Plan> {
    let plan = state.plans.create(dto, claims.user_id_uuid()).await?;
    created(plan)
}

#[utoipa::path(
    patch,
    path = "/plans/{id}",
    params(
        ("id" = Uuid, Path, description = "Plan ID"),
    ),
    request_body = UpdatePlanDto,
    responses(
        (status = 200, description = "Update plan", body = PlanEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "plans"
)]
pub async fn update_plan(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdatePlanDto>,
) -> ApiResult<Plan> {
    let plan = state.plans.update(id, dto, claims.user_id_uuid()).await?;
    ok(plan)
}

#[utoipa::path(
    delete,
    path = "/plans/{id}",
    params(
        ("id" = Uuid, Path, description = "Plan ID"),
    ),
    responses(
        (status = 204, description = "Soft-deactivated (isActive=false)"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "plans"
)]
pub async fn delete_plan(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, crate::error::AppError> {
    state.plans.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

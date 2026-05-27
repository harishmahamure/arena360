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
use crate::models::{CreatePlanDto, Plan, PlanFilterDto, UpdatePlanDto};
use crate::openapi::responses::{
    ActivePlansEnvelope, ErrorEnvelope, PlanEnvelope, PlanPaginationEnvelope,
};

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
    State(state): State<Arc<AppState>>,
    Query(filters): Query<PlanFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<Plan>> {
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
pub async fn get_active_plans(State(state): State<Arc<AppState>>) -> ApiResult<Vec<Plan>> {
    let plans = state.plans.get_active().await?;
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
pub async fn get_plan(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Plan> {
    let plan = state.plans.get_by_id(id).await?;
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
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreatePlanDto>,
) -> ApiResult<Plan> {
    let plan = state.plans.create(dto).await?;
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
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdatePlanDto>,
) -> ApiResult<Plan> {
    let plan = state.plans.update(id, dto).await?;
    ok(plan)
}

#[utoipa::path(
    delete,
    path = "/plans/{id}",
    params(
        ("id" = Uuid, Path, description = "Plan ID"),
    ),
    responses(
        (status = 204, description = "Deleted"),
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

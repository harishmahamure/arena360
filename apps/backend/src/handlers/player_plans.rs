use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::middleware::{AdminOrStaff, AuthUser};
use crate::models::{
    status, AssignPlanDto, PlayerPlan, PlayerPlanFilterDto, ValidationResult, PlayerPlanResponse,
};
use crate::openapi::responses::{
    ErrorEnvelope, PlayerPlanEnvelope, PlayerPlanFlatEnvelope, PlayerPlanPaginationEnvelope,
    ValidationResultEnvelope,
};
use crate::services::PlayerPlanService;

#[utoipa::path(
    get,
    path = "/player-plans",
    responses(
        (status = 200, description = "List player plans", body = PlayerPlanPaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "player-plans"
)]
pub async fn list_player_plans(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<PlayerPlanFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<PlayerPlanResponse>> {
    let filters = PlayerPlanService::enforce_player_scope(
        filters,
        &claims.userId,
        claims.is_admin_or_staff(),
    )?;
    let result = state.player_plans.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/player-plans/my-active-plans",
    responses(
        (status = 200, description = "List my active player plans", body = PlayerPlanPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "player-plans"
)]
pub async fn list_my_active_plans(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Query(mut filters): Query<PlayerPlanFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<PlayerPlanResponse>> {
    let user_uuid = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::Unauthorized("Authentication required".to_string()))?;
    filters.player_id = Some(user_uuid);
    filters.status = Some(status::ACTIVE.to_string());
    let result = state.player_plans.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/player-plans/best-plan",
    responses(
        (status = 200, description = "Get best active player plan", body = PlayerPlanFlatEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "player-plans"
)]
pub async fn get_best_plan(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<PlayerPlan> {
    let player_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::Unauthorized("Authentication required".to_string()))?;
    let player_plan = state.player_plans.get_best_plan(player_id).await?;
    ok(player_plan)
}

#[utoipa::path(
    post,
    path = "/player-plans",
    request_body = AssignPlanDto,
    responses(
        (status = 201, description = "Assign plan to player", body = PlayerPlanFlatEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "player-plans"
)]
pub async fn assign_plan(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<AssignPlanDto>,
) -> ApiResult<PlayerPlan> {
    let player_plan = state.player_plans.assign_plan_to_player(dto).await?;
    created(player_plan)
}

#[utoipa::path(
    get,
    path = "/player-plans/{id}",
    params(
        ("id" = Uuid, Path, description = "Player plan ID"),
    ),
    responses(
        (status = 200, description = "Get player plan", body = PlayerPlanEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "player-plans"
)]
pub async fn get_player_plan(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<PlayerPlanResponse> {
    let player_plan = state.player_plans.get_by_id(id).await?;
    PlayerPlanService::ensure_owner_or_admin(
        &claims.userId,
        claims.is_admin_or_staff(),
        player_plan.player_id,
    )?;
    ok(player_plan)
}

#[utoipa::path(
    post,
    path = "/player-plans/{id}/validate",
    params(
        ("id" = Uuid, Path, description = "Player plan ID"),
    ),
    responses(
        (status = 200, description = "Validate player plan access", body = ValidationResultEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "player-plans"
)]
pub async fn validate_access(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<ValidationResult> {
    let player_plan = state.player_plans.get_by_id(id).await?;
    PlayerPlanService::ensure_owner_or_admin(
        &claims.userId,
        claims.is_admin_or_staff(),
        player_plan.player_id,
    )?;
    let result = state.player_plans.validate_plan_access(id, None).await?;
    ok(result)
}

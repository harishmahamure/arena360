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
    balance_status, BalanceFilterDto, BalanceValidationResult, PlayerPlanBalance,
    PlayerPlanBalanceResponse, PurchaseBalanceDto,
};
use crate::openapi::responses::{
    BalanceEnvelope, BalanceFlatEnvelope, BalancePaginationEnvelope, BalanceValidationEnvelope,
    ErrorEnvelope,
};
use crate::services::BalanceService;

#[utoipa::path(
    get,
    path = "/player-plans",
    responses(
        (status = 200, description = "List player plan balances", body = BalancePaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "player-plans"
)]
pub async fn list_balances(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<BalanceFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<PlayerPlanBalanceResponse>> {
    let filters =
        BalanceService::enforce_player_scope(filters, &claims.userId, claims.is_admin_or_staff())?;
    let result = state.balances.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/player-plans/my-active-plans",
    responses(
        (status = 200, description = "List my active balances", body = BalancePaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "player-plans"
)]
pub async fn list_my_active_balances(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Query(mut filters): Query<BalanceFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<PlayerPlanBalanceResponse>> {
    let user_uuid = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::Unauthorized("Authentication required".to_string()))?;
    filters.player_id = Some(user_uuid);
    filters.status = Some(balance_status::ACTIVE.to_string());
    filters.usable_only = Some(true);
    let result = state.balances.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/player-plans/best-plan",
    responses(
        (status = 200, description = "Get best active balance", body = BalanceFlatEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "player-plans"
)]
pub async fn get_best_balance(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<PlayerPlanBalance> {
    let player_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::Unauthorized("Authentication required".to_string()))?;
    let balance = state.balances.get_best_balance(player_id).await?;
    ok(balance)
}

#[utoipa::path(
    post,
    path = "/player-plans",
    request_body = PurchaseBalanceDto,
    responses(
        (status = 201, description = "Purchase or recharge a balance", body = BalanceFlatEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "player-plans"
)]
pub async fn purchase_balance(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<PurchaseBalanceDto>,
) -> ApiResult<PlayerPlanBalance> {
    let balance = state
        .balances
        .purchase_or_recharge(dto, claims.user_id_uuid())
        .await?;
    created(balance)
}

#[utoipa::path(
    get,
    path = "/player-plans/{id}",
    params(
        ("id" = Uuid, Path, description = "Balance ID"),
    ),
    responses(
        (status = 200, description = "Get balance", body = BalanceEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "player-plans"
)]
pub async fn get_balance(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<PlayerPlanBalanceResponse> {
    let balance = state.balances.get_by_id(id).await?;
    BalanceService::ensure_owner_or_admin(
        &claims.userId,
        claims.is_admin_or_staff(),
        balance.player_id,
    )?;
    ok(balance)
}

#[utoipa::path(
    post,
    path = "/player-plans/{id}/validate",
    params(
        ("id" = Uuid, Path, description = "Balance ID"),
    ),
    responses(
        (status = 200, description = "Validate balance access", body = BalanceValidationEnvelope),
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
) -> ApiResult<BalanceValidationResult> {
    let balance = state.balances.get_by_id(id).await?;
    BalanceService::ensure_owner_or_admin(
        &claims.userId,
        claims.is_admin_or_staff(),
        balance.player_id,
    )?;
    let result = state.balances.validate_access(id, None, None).await?;
    ok(result)
}

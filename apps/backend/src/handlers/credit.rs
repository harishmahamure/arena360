use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult, PaginationResult};
use crate::error::AppError;
use crate::middleware::{require_staff_for_counter, AdminOrStaff, AdminUser};
use crate::models::{
    CreditAccountFilterDto, CreditPlayerRow, CreditSettlement, CreditSettlementDetail,
    CreditSettlementFilterDto, CreditSummary, PlayerCreditDetail, SetCreditLimitDto,
    SettleCreditDto,
};
use crate::openapi::responses::{
    CreditPlayerPaginationEnvelope, CreditSettlementDetailEnvelope,
    CreditSettlementEnvelope, CreditSettlementPaginationEnvelope, CreditSummaryEnvelope,
    ErrorEnvelope, PlayerCreditDetailEnvelope,
};

#[utoipa::path(
    get,
    path = "/credit/accounts",
    params(CreditAccountFilterDto),
    responses(
        (status = 200, description = "List players with outstanding credit", body = CreditPlayerPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "credit"
)]
pub async fn list_credit_accounts(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<CreditAccountFilterDto>,
) -> ApiResult<PaginationResult<CreditPlayerRow>> {
    let result = state.credit.list_credit_players(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/credit/players/{id}",
    params(
        ("id" = Uuid, Path, description = "Player ID"),
    ),
    responses(
        (status = 200, description = "Player credit summary and outstanding transactions", body = PlayerCreditDetailEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "credit"
)]
pub async fn get_player_credit(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<PlayerCreditDetail> {
    let detail = state.credit.get_player_credit(id).await?;
    ok(detail)
}

#[utoipa::path(
    get,
    path = "/credit/settlements",
    params(CreditSettlementFilterDto),
    responses(
        (status = 200, description = "List credit settlements", body = CreditSettlementPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "credit"
)]
pub async fn list_settlements(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<CreditSettlementFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<crate::models::CreditSettlementListRow>> {
    let result = state.credit.list_settlements(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/credit/settlements/{id}",
    params(
        ("id" = Uuid, Path, description = "Settlement ID"),
    ),
    responses(
        (status = 200, description = "Credit settlement detail", body = CreditSettlementDetailEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "credit"
)]
pub async fn get_settlement(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<CreditSettlementDetail> {
    let detail = state.credit.get_settlement(id).await?;
    ok(detail)
}

#[utoipa::path(
    post,
    path = "/credit/settlements",
    request_body = SettleCreditDto,
    responses(
        (status = 201, description = "Credit settlement recorded", body = CreditSettlementEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "credit"
)]
pub async fn create_settlement(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<SettleCreditDto>,
) -> ApiResult<CreditSettlement> {
    let user_id = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::BadRequest("Invalid user ID in token".to_string()))?;

    require_staff_for_counter(&claims)?;

    let active_shift = state.shifts.get_active(user_id).await?.ok_or_else(|| {
        AppError::BadRequest("No active shift found for current user".to_string())
    })?;

    let settlement = state
        .credit
        .settle(dto, active_shift.id, user_id, &state.cash_registers)
        .await?;
    created(settlement)
}

#[utoipa::path(
    patch,
    path = "/users/{id}/credit-limit",
    params(
        ("id" = Uuid, Path, description = "Player ID"),
    ),
    request_body = SetCreditLimitDto,
    responses(
        (status = 200, description = "Credit limit updated", body = CreditSummaryEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "credit"
)]
pub async fn update_credit_limit(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<SetCreditLimitDto>,
) -> ApiResult<CreditSummary> {
    let summary = state
        .credit
        .set_limit(id, dto, claims.user_id_uuid())
        .await?;
    ok(summary)
}

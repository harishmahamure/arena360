use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult, PaginationResult};
use crate::error::AppError;
use crate::middleware::{AdminOrStaff, AdminUser, StaffUser};
use crate::models::{
    ApproveDepositDto, CashDeposit, CashDepositFilterDto, InitiateDepositDto, RejectDepositDto,
};
use crate::openapi::responses::{
    CashDepositEnvelope, CashDepositPaginationEnvelope, ErrorEnvelope,
};

#[utoipa::path(
    post,
    path = "/cash-deposits",
    request_body = InitiateDepositDto,
    responses(
        (status = 201, description = "Deposit initiated", body = CashDepositEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "cash-deposits"
)]
pub async fn initiate_deposit(
    StaffUser(claims): StaffUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<InitiateDepositDto>,
) -> ApiResult<CashDeposit> {
    let staff_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| AppError::BadRequest("Invalid user ID in token".to_string()))?;
    let deposit = state.cash_deposits.initiate(dto, staff_id).await?;
    created(deposit)
}

#[utoipa::path(
    get,
    path = "/cash-deposits",
    params(CashDepositFilterDto),
    responses(
        (status = 200, description = "List cash deposits", body = CashDepositPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "cash-deposits"
)]
pub async fn list_deposits(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<CashDepositFilterDto>,
) -> ApiResult<PaginationResult<CashDeposit>> {
    let result = state.cash_deposits.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/cash-deposits/{id}",
    params(
        ("id" = Uuid, Path, description = "Cash deposit ID"),
    ),
    responses(
        (status = 200, description = "Get cash deposit", body = CashDepositEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "cash-deposits"
)]
pub async fn get_deposit(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<CashDeposit> {
    let deposit = state.cash_deposits.get_by_id(id).await?;
    ok(deposit)
}

#[utoipa::path(
    patch,
    path = "/cash-deposits/{id}/approve",
    params(
        ("id" = Uuid, Path, description = "Cash deposit ID"),
    ),
    request_body = ApproveDepositDto,
    responses(
        (status = 200, description = "Deposit approved", body = CashDepositEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "cash-deposits"
)]
pub async fn approve_deposit(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<ApproveDepositDto>,
) -> ApiResult<CashDeposit> {
    let admin_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| AppError::BadRequest("Invalid user ID in token".to_string()))?;
    let deposit = state
        .cash_deposits
        .approve(id, &dto.deposit_type, admin_id)
        .await?;
    ok(deposit)
}

#[utoipa::path(
    patch,
    path = "/cash-deposits/{id}/reject",
    params(
        ("id" = Uuid, Path, description = "Cash deposit ID"),
    ),
    request_body = RejectDepositDto,
    responses(
        (status = 200, description = "Deposit rejected", body = CashDepositEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "cash-deposits"
)]
pub async fn reject_deposit(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<RejectDepositDto>,
) -> ApiResult<CashDeposit> {
    let admin_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| AppError::BadRequest("Invalid user ID in token".to_string()))?;
    let deposit = state
        .cash_deposits
        .reject(id, &dto.rejection_reason, admin_id)
        .await?;
    ok(deposit)
}

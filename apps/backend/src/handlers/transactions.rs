use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::middleware::AdminOrStaff;
use crate::models::{
    CreateTransactionDto, Transaction, TransactionFilterDto, TransactionWithLineItems,
    UpdateTransactionDto,
};
use crate::openapi::responses::{
    ErrorEnvelope, TransactionEnvelope, TransactionPaginationEnvelope,
    TransactionWithLineItemsEnvelope,
};

#[utoipa::path(
    get,
    path = "/transactions",
    responses(
        (status = 200, description = "List transactions", body = TransactionPaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "transactions"
)]
pub async fn list_transactions(
    State(state): State<Arc<AppState>>,
    Query(filters): Query<TransactionFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<Transaction>> {
    let result = state.transactions.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/transactions/{id}",
    params(
        ("id" = Uuid, Path, description = "Transaction ID"),
    ),
    responses(
        (status = 200, description = "Get transaction with line items", body = TransactionWithLineItemsEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "transactions"
)]
pub async fn get_transaction(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<TransactionWithLineItems> {
    let result = state.transactions.get_by_id_with_items(id).await?;
    ok(result)
}

#[utoipa::path(
    post,
    path = "/transactions",
    request_body = CreateTransactionDto,
    responses(
        (status = 201, description = "Create transaction", body = TransactionEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "transactions"
)]
pub async fn create_transaction(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(mut dto): Json<CreateTransactionDto>,
) -> ApiResult<Transaction> {
    let user_id = claims.user_id_uuid().ok_or_else(|| {
        crate::error::AppError::BadRequest("Invalid user ID in token".to_string())
    })?;

    // Enforce active shift
    let active_shift = state.shifts.get_active(user_id).await?.ok_or_else(|| {
        crate::error::AppError::BadRequest("No active shift found for current user".to_string())
    })?;

    dto.shift_id = Some(active_shift.id);

    let transaction = state
        .transactions
        .create(dto, Some(user_id), &state.cash_registers)
        .await?;
    created(transaction)
}

#[utoipa::path(
    patch,
    path = "/transactions/{id}",
    params(
        ("id" = Uuid, Path, description = "Transaction ID"),
    ),
    request_body = UpdateTransactionDto,
    responses(
        (status = 200, description = "Update transaction", body = TransactionEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "transactions"
)]
pub async fn update_transaction(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateTransactionDto>,
) -> ApiResult<Transaction> {
    let transaction = state
        .transactions
        .update(id, dto, claims.user_id_uuid(), &state.cash_registers)
        .await?;
    ok(transaction)
}

use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::middleware::AdminUser;
use crate::models::{CreateTransactionDto, Transaction, TransactionFilterDto};
use crate::openapi::responses::{
    ErrorEnvelope, TransactionEnvelope, TransactionPaginationEnvelope,
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
        (status = 200, description = "Get transaction", body = TransactionEnvelope),
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
) -> ApiResult<Transaction> {
    let transaction = state.transactions.get_by_id(id).await?;
    ok(transaction)
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
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateTransactionDto>,
) -> ApiResult<Transaction> {
    let transaction = state.transactions.create(dto).await?;
    created(transaction)
}

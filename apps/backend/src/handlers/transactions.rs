use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::middleware::{require_staff_for_counter, AdminOrStaff};
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
) -> ApiResult<crate::dto::PaginationResult<crate::models::TransactionResponse>> {
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

    let org = Uuid::parse_str(&claims.tenantId)
        .map_err(|_| crate::error::AppError::Forbidden("Select an organization".into()))?;
    let venue_id = dto.venue_location_id.unwrap_or(crate::models::DEFAULT_VENUE_LOCATION_ID);
    state.config.ensure_location_permission(
        org,
        venue_id,
        user_id,
        "transactions:write",
    ).await?;
    if let Some(order_id) = dto.kiosk_order_id {
        let order_venue: Option<Uuid> = sqlx::query_scalar(r#"SELECT d."locationId" FROM kiosk_orders o JOIN devices d ON d.id=o."deviceId" WHERE o.id=$1"#)
            .bind(order_id).fetch_optional(&state.db).await?;
        if order_venue.is_some_and(|location| location != venue_id) {
            return Err(crate::error::AppError::BadRequest("Kiosk order belongs to another location".into()));
        }
    }

    require_staff_for_counter(&claims)?;

    // Enforce active shift
    let active_shift = state.shifts.get_active(user_id).await?.ok_or_else(|| {
        crate::error::AppError::BadRequest("No active shift found for current user".to_string())
    })?;
    let shift_venue: Uuid = sqlx::query_scalar(r#"SELECT "venueLocationId" FROM shifts WHERE id=$1"#)
        .bind(active_shift.id).fetch_one(&state.db).await?;
    if shift_venue != venue_id {
        return Err(crate::error::AppError::Forbidden("Start a shift at the selected location before checkout".into()));
    }

    dto.shift_id = Some(active_shift.id);

    let kiosk_order_id = dto.kiosk_order_id;
    let actor_role = claims.roles.first().map(|s| s.as_str());
    let transaction = state
        .transactions
        .create(dto, Some(user_id), actor_role, &state.cash_registers)
        .await?;

    if let Some(order_id) = kiosk_order_id {
        state
            .kiosk_orders
            .mark_fulfilled(order_id, transaction.id)
            .await?;
    }

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

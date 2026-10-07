use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::access::scope::{requested_location, LocationScope, TransactionReadScope};
use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::middleware::{require_staff_for_counter, AdminOrStaff, AuthUser};
use crate::models::{
    CreateTransactionDto, Transaction, TransactionFilterDto, TransactionWithLineItems,
    UpdateTransactionDto,
};
use crate::openapi::responses::{
    ErrorEnvelope, TransactionEnvelope, TransactionPaginationEnvelope,
    TransactionWithLineItemsEnvelope,
};
use crate::repositories::TenantTransactionRepository;

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
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(mut filters): Query<TransactionFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<crate::models::TransactionResponse>> {
    let db = state.business_db(&claims).await?;
    let scope =
        TransactionReadScope::resolve_tenant(db.clone(), &claims, requested_location(&headers)?)
            .await?;
    scope.apply_player_filter(&mut filters)?;
    ok(TenantTransactionRepository::new(db)
        .list_scoped(&filters, scope.locations.as_deref())
        .await?)
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
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<TransactionWithLineItems> {
    let db = state.business_db(&claims).await?;
    let scope = TransactionReadScope::resolve_tenant(db.clone(), &claims, None).await?;
    let repo = TenantTransactionRepository::new(db);
    let (player, venue) = repo.access_context(id).await?;
    scope.ensure_resource(player, venue)?;
    ok(repo.get_with_items(id).await?)
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

    let db = state.business_db(&claims).await?;
    let venue_id = dto
        .venue_location_id
        .ok_or_else(|| crate::error::AppError::BadRequest("venueLocationId is required".into()))?;
    crate::repositories::TenantSettingsRepository::new(db.clone())
        .ensure_location_permission(db.tenant_id(), venue_id, user_id, "transactions:write")
        .await?;
    require_staff_for_counter(&claims)?;
    let active_shift = crate::repositories::TenantShiftRepository::new(db.clone())
        .find_active_by_user(user_id)
        .await?
        .ok_or_else(|| {
            crate::error::AppError::BadRequest("No active shift found for current user".into())
        })?;
    let shift_venue: String = sqlx::query_scalar("SELECT location_id FROM shifts WHERE id=?")
        .bind(active_shift.id.to_string())
        .fetch_one(&db.read_pool()?)
        .await?;
    if shift_venue != venue_id.to_string() {
        return Err(crate::error::AppError::Forbidden(
            "Start a shift at the selected location before checkout".into(),
        ));
    }
    dto.shift_id = Some(active_shift.id);
    let transaction = state
        .transactions
        .create_tenant(db, dto, Some(user_id))
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
    let db = state.business_db(&claims).await?;
    let (_, venue) = TenantTransactionRepository::new(db.clone())
        .access_context(id)
        .await?;
    LocationScope::resolve_tenant(db.clone(), &claims, "transactions:write", Some(venue)).await?;
    let transaction = state
        .transactions
        .update_tenant(db, id, dto, claims.user_id_uuid())
        .await?;
    ok(transaction)
}

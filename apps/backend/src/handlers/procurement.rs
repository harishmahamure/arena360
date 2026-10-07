use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult, PaginationResult};
use crate::error::AppError;
use crate::middleware::{AdminOrStaff, AdminUser};
use crate::models::{
    CreatePurchaseOrderDto, InventoryOverviewDto, InventoryReorderRule, PurchaseOrder,
    PurchaseOrderFilterDto, PurchaseOrderWithLines, ReceivePurchaseOrderDto,
    ReceivePurchaseOrderResponse, RejectPurchaseOrderDto, ReorderSuggestion,
    StockMovementFilterDto, StockMovementRow, UpdatePurchaseOrderDto,
    UpsertInventoryReorderRuleDto,
};
use crate::openapi::responses::{
    ErrorEnvelope, InventoryOverviewEnvelope, InventoryReorderRuleEnvelope,
    InventoryReorderRuleListEnvelope, PurchaseOrderEnvelope, PurchaseOrderPaginationEnvelope,
    PurchaseOrderReceiptEnvelope, ReorderSuggestionListEnvelope, StockMovementPaginationEnvelope,
};
use crate::services::TenantProcurementService;

async fn service(
    state: &AppState,
    claims: &crate::dto::JwtUserClaims,
) -> Result<TenantProcurementService, AppError> {
    let db = state.business_db(claims).await?;
    let timezone = db.timezone().await?;
    Ok(TenantProcurementService::new(db, timezone))
}

fn actor_id(claims: &crate::dto::JwtUserClaims) -> Result<Uuid, AppError> {
    claims
        .userId
        .parse()
        .map_err(|_| AppError::BadRequest("Invalid user ID in token".to_string()))
}

#[utoipa::path(get, operation_id = "inventory_overview", path = "/inventory/overview", responses((status = 200, body = InventoryOverviewEnvelope), (status = 401, body = ErrorEnvelope)), security(("bearer_auth" = [])), tag = "procurement")]
pub async fn overview(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
) -> ApiResult<InventoryOverviewDto> {
    ok(service(&state, &claims).await?.overview().await?)
}

#[utoipa::path(get, operation_id = "inventory_list_movements", path = "/inventory/movements", params(StockMovementFilterDto), responses((status = 200, body = StockMovementPaginationEnvelope), (status = 401, body = ErrorEnvelope)), security(("bearer_auth" = [])), tag = "procurement")]
pub async fn list_movements(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<StockMovementFilterDto>,
) -> ApiResult<PaginationResult<StockMovementRow>> {
    ok(service(&state, &claims)
        .await?
        .list_movements(filters)
        .await?)
}

#[utoipa::path(get, operation_id = "purchase_orders_list", path = "/inventory/purchase-orders", params(PurchaseOrderFilterDto), responses((status = 200, body = PurchaseOrderPaginationEnvelope), (status = 401, body = ErrorEnvelope)), security(("bearer_auth" = [])), tag = "procurement")]
pub async fn list_orders(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<PurchaseOrderFilterDto>,
) -> ApiResult<PaginationResult<PurchaseOrder>> {
    ok(service(&state, &claims).await?.list_orders(filters).await?)
}

#[utoipa::path(get, operation_id = "purchase_orders_get", path = "/inventory/purchase-orders/{id}", params(("id" = Uuid, Path)), responses((status = 200, body = PurchaseOrderEnvelope), (status = 404, body = ErrorEnvelope)), security(("bearer_auth" = [])), tag = "procurement")]
pub async fn get_order(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<PurchaseOrderWithLines> {
    ok(service(&state, &claims).await?.get_order(id).await?)
}

#[utoipa::path(post, operation_id = "purchase_orders_create", path = "/inventory/purchase-orders", request_body = CreatePurchaseOrderDto, responses((status = 201, body = PurchaseOrderEnvelope), (status = 400, body = ErrorEnvelope)), security(("bearer_auth" = [])), tag = "procurement")]
pub async fn create_order(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreatePurchaseOrderDto>,
) -> ApiResult<PurchaseOrderWithLines> {
    created(
        service(&state, &claims)
            .await?
            .create_order(dto, actor_id(&claims)?)
            .await?,
    )
}

#[utoipa::path(patch, operation_id = "purchase_orders_update", path = "/inventory/purchase-orders/{id}", params(("id" = Uuid, Path)), request_body = UpdatePurchaseOrderDto, responses((status = 200, body = PurchaseOrderEnvelope), (status = 409, body = ErrorEnvelope)), security(("bearer_auth" = [])), tag = "procurement")]
pub async fn update_order(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdatePurchaseOrderDto>,
) -> ApiResult<PurchaseOrderWithLines> {
    ok(service(&state, &claims)
        .await?
        .update_order(id, dto, actor_id(&claims)?)
        .await?)
}

#[utoipa::path(post, path = "/inventory/purchase-orders/{id}/submit", params(("id" = Uuid, Path)), responses((status = 200, body = PurchaseOrderEnvelope), (status = 409, body = ErrorEnvelope)), security(("bearer_auth" = [])), tag = "procurement")]
pub async fn submit_order(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<PurchaseOrderWithLines> {
    ok(service(&state, &claims)
        .await?
        .transition(id, "submit", None, actor_id(&claims)?)
        .await?)
}

#[utoipa::path(post, path = "/inventory/purchase-orders/{id}/approve", params(("id" = Uuid, Path)), responses((status = 200, body = PurchaseOrderEnvelope), (status = 403, body = ErrorEnvelope), (status = 409, body = ErrorEnvelope)), security(("bearer_auth" = [])), tag = "procurement")]
pub async fn approve_order(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<PurchaseOrderWithLines> {
    ok(service(&state, &claims)
        .await?
        .transition(id, "approve", None, actor_id(&claims)?)
        .await?)
}

#[utoipa::path(post, path = "/inventory/purchase-orders/{id}/reject", params(("id" = Uuid, Path)), request_body = RejectPurchaseOrderDto, responses((status = 200, body = PurchaseOrderEnvelope), (status = 403, body = ErrorEnvelope), (status = 409, body = ErrorEnvelope)), security(("bearer_auth" = [])), tag = "procurement")]
pub async fn reject_order(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<RejectPurchaseOrderDto>,
) -> ApiResult<PurchaseOrderWithLines> {
    ok(service(&state, &claims)
        .await?
        .transition(id, "reject", Some(dto.reason), actor_id(&claims)?)
        .await?)
}

#[utoipa::path(post, path = "/inventory/purchase-orders/{id}/mark-ordered", params(("id" = Uuid, Path)), responses((status = 200, body = PurchaseOrderEnvelope), (status = 409, body = ErrorEnvelope)), security(("bearer_auth" = [])), tag = "procurement")]
pub async fn mark_ordered(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<PurchaseOrderWithLines> {
    ok(service(&state, &claims)
        .await?
        .transition(id, "mark_ordered", None, actor_id(&claims)?)
        .await?)
}

#[utoipa::path(post, path = "/inventory/purchase-orders/{id}/cancel", params(("id" = Uuid, Path)), responses((status = 200, body = PurchaseOrderEnvelope), (status = 403, body = ErrorEnvelope), (status = 409, body = ErrorEnvelope)), security(("bearer_auth" = [])), tag = "procurement")]
pub async fn cancel_order(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<PurchaseOrderWithLines> {
    ok(service(&state, &claims)
        .await?
        .transition(id, "cancel", None, actor_id(&claims)?)
        .await?)
}

#[utoipa::path(post, path = "/inventory/purchase-orders/{id}/receipts", params(("id" = Uuid, Path)), request_body = ReceivePurchaseOrderDto, responses((status = 201, body = PurchaseOrderReceiptEnvelope), (status = 400, body = ErrorEnvelope), (status = 409, body = ErrorEnvelope)), security(("bearer_auth" = [])), tag = "procurement")]
pub async fn receive_order(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<ReceivePurchaseOrderDto>,
) -> ApiResult<ReceivePurchaseOrderResponse> {
    created(
        service(&state, &claims)
            .await?
            .receive(id, dto, actor_id(&claims)?)
            .await?,
    )
}

#[utoipa::path(get, path = "/inventory/reorder-rules", responses((status = 200, body = InventoryReorderRuleListEnvelope), (status = 401, body = ErrorEnvelope)), security(("bearer_auth" = [])), tag = "procurement")]
pub async fn list_reorder_rules(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Vec<InventoryReorderRule>> {
    ok(service(&state, &claims).await?.list_reorder_rules().await?)
}

#[utoipa::path(post, path = "/inventory/reorder-rules", request_body = UpsertInventoryReorderRuleDto, responses((status = 200, body = InventoryReorderRuleEnvelope), (status = 400, body = ErrorEnvelope), (status = 403, body = ErrorEnvelope)), security(("bearer_auth" = [])), tag = "procurement")]
pub async fn upsert_reorder_rule(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<UpsertInventoryReorderRuleDto>,
) -> ApiResult<InventoryReorderRule> {
    ok(service(&state, &claims)
        .await?
        .upsert_reorder_rule(dto, actor_id(&claims)?)
        .await?)
}

#[utoipa::path(get, path = "/inventory/reorder-suggestions", responses((status = 200, body = ReorderSuggestionListEnvelope), (status = 401, body = ErrorEnvelope)), security(("bearer_auth" = [])), tag = "procurement")]
pub async fn reorder_suggestions(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Vec<ReorderSuggestion>> {
    ok(service(&state, &claims)
        .await?
        .reorder_suggestions()
        .await?)
}

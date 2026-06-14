use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{
    created, ok, ApiResult, ApproveInventoryActionDto, PaginationResult, WasteSummaryFilterDto,
};
use crate::middleware::{AdminOrStaff, AdminUser};
use crate::models::{
    CreateInventoryLocationDto, CreateStockReceiptDto, CreateStockTransferRequestDto,
    CreateStockWasteEventDto, CreateStockAdjustmentDto, InventoryLocation, InventoryLocationFilterDto,
    LocationStockFilterDto, LocationStockRow, RejectStockTransferDto, RejectStockWasteDto,
    StockReceipt, StockReceiptFilterDto, StockReceiptWithLines, StockAdjustment,
    StockAdjustmentFilterDto, StockAdjustmentWithLines, StockTransferFilterDto,
    StockTransferRequest, StockTransferRequestWithLines, StockWasteEvent,
    StockWasteEventWithLines, StockWasteFilterDto, UpdateInventoryLocationDto, WasteSummaryRow,
};
use crate::openapi::responses::{
    ErrorEnvelope, InventoryLocationEnvelope, InventoryLocationPaginationEnvelope,
    LocationStockPaginationEnvelope, StockReceiptPaginationEnvelope,
    StockReceiptWithLinesEnvelope, StockAdjustmentEnvelope, StockAdjustmentPaginationEnvelope,
    StockAdjustmentWithLinesEnvelope, StockTransferEnvelope, StockTransferPaginationEnvelope,
    StockTransferWithLinesEnvelope, StockWasteEnvelope, StockWastePaginationEnvelope,
    StockWasteSummaryListEnvelope, StockWasteWithLinesEnvelope,
};

#[utoipa::path(
    get,
    path = "/inventory/locations",
    responses(
        (status = 200, description = "List inventory locations", body = InventoryLocationPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn list_locations(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<InventoryLocationFilterDto>,
) -> ApiResult<PaginationResult<InventoryLocation>> {
    ok(state.inventory.list_locations(filters).await?)
}

#[utoipa::path(
    post,
    path = "/inventory/locations",
    request_body = CreateInventoryLocationDto,
    responses(
        (status = 201, description = "Create inventory location", body = InventoryLocationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn create_location(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateInventoryLocationDto>,
) -> ApiResult<InventoryLocation> {
    let user_id = Uuid::parse_str(&claims.userId).ok();
    created(state.inventory.create_location(dto, user_id).await?)
}

#[utoipa::path(
    patch,
    path = "/inventory/locations/{id}",
    params(("id" = Uuid, Path, description = "Location ID")),
    request_body = UpdateInventoryLocationDto,
    responses(
        (status = 200, description = "Update inventory location", body = InventoryLocationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn update_location(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateInventoryLocationDto>,
) -> ApiResult<InventoryLocation> {
    let user_id = Uuid::parse_str(&claims.userId).ok();
    ok(state.inventory.update_location(id, dto, user_id).await?)
}

#[utoipa::path(
    get,
    path = "/inventory/stock",
    responses(
        (status = 200, description = "List location stock", body = LocationStockPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn list_stock(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<LocationStockFilterDto>,
) -> ApiResult<PaginationResult<LocationStockRow>> {
    ok(state.inventory.list_stock(filters).await?)
}

#[utoipa::path(
    post,
    path = "/inventory/receipts",
    request_body = CreateStockReceiptDto,
    responses(
        (status = 201, description = "Receive stock", body = StockReceiptWithLinesEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn create_receipt(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateStockReceiptDto>,
) -> ApiResult<StockReceiptWithLines> {
    let user_id = Uuid::parse_str(&claims.userId).ok();
    created(state.inventory.receive_stock(dto, user_id).await?)
}

#[utoipa::path(
    get,
    path = "/inventory/receipts",
    responses(
        (status = 200, description = "List stock receipts", body = StockReceiptPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn list_receipts(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<StockReceiptFilterDto>,
) -> ApiResult<PaginationResult<StockReceipt>> {
    ok(state.inventory.list_receipts(filters).await?)
}

#[utoipa::path(
    post,
    path = "/inventory/adjustments",
    request_body = CreateStockAdjustmentDto,
    responses(
        (status = 201, description = "Reconcile stock count", body = StockAdjustmentWithLinesEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn create_adjustment(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateStockAdjustmentDto>,
) -> ApiResult<StockAdjustmentWithLines> {
    let user_id = Uuid::parse_str(&claims.userId).ok();
    created(state.inventory.create_adjustment(dto, user_id).await?)
}

#[utoipa::path(
    get,
    path = "/inventory/adjustments",
    responses(
        (status = 200, description = "List stock adjustments", body = StockAdjustmentPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn list_adjustments(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<StockAdjustmentFilterDto>,
) -> ApiResult<PaginationResult<StockAdjustment>> {
    ok(state.inventory.list_adjustments(filters).await?)
}

#[utoipa::path(
    get,
    path = "/inventory/adjustments/{id}",
    params(("id" = Uuid, Path, description = "Adjustment ID")),
    responses(
        (status = 200, description = "Get stock adjustment", body = StockAdjustmentWithLinesEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn get_adjustment(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StockAdjustmentWithLines> {
    ok(state.inventory.get_adjustment(id).await?)
}

#[utoipa::path(
    post,
    path = "/inventory/transfer-requests",
    request_body = CreateStockTransferRequestDto,
    responses(
        (status = 201, description = "Create transfer request", body = StockTransferWithLinesEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn create_transfer_request(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateStockTransferRequestDto>,
) -> ApiResult<StockTransferRequestWithLines> {
    let user_id = Uuid::parse_str(&claims.userId).ok();
    created(
        state
            .inventory
            .create_transfer_request(dto, user_id)
            .await?,
    )
}

#[utoipa::path(
    get,
    path = "/inventory/transfer-requests",
    responses(
        (status = 200, description = "List transfer requests", body = StockTransferPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn list_transfer_requests(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<StockTransferFilterDto>,
) -> ApiResult<PaginationResult<StockTransferRequest>> {
    ok(state.inventory.list_transfer_requests(filters).await?)
}

#[utoipa::path(
    get,
    path = "/inventory/transfer-requests/{id}",
    params(("id" = Uuid, Path, description = "Transfer request ID")),
    responses(
        (status = 200, description = "Get transfer request", body = StockTransferWithLinesEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn get_transfer_request(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StockTransferRequestWithLines> {
    ok(state.inventory.get_transfer_request(id).await?)
}

#[utoipa::path(
    patch,
    path = "/inventory/transfer-requests/{id}/approve",
    params(("id" = Uuid, Path, description = "Transfer request ID")),
    request_body = ApproveInventoryActionDto,
    responses(
        (status = 200, description = "Approve transfer request", body = StockTransferEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn approve_transfer_request(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(_dto): Json<ApproveInventoryActionDto>,
) -> ApiResult<StockTransferRequest> {
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    ok(state.inventory.approve_transfer(id, user_id).await?)
}

#[utoipa::path(
    patch,
    path = "/inventory/transfer-requests/{id}/reject",
    params(("id" = Uuid, Path, description = "Transfer request ID")),
    request_body = RejectStockTransferDto,
    responses(
        (status = 200, description = "Reject transfer request", body = StockTransferEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn reject_transfer_request(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<RejectStockTransferDto>,
) -> ApiResult<StockTransferRequest> {
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    ok(
        state
            .inventory
            .reject_transfer(id, &dto.rejection_reason, user_id)
            .await?,
    )
}

#[utoipa::path(
    patch,
    path = "/inventory/transfer-requests/{id}/fulfill",
    params(("id" = Uuid, Path, description = "Transfer request ID")),
    request_body = ApproveInventoryActionDto,
    responses(
        (status = 200, description = "Fulfill transfer request", body = StockTransferEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 409, description = "Insufficient stock", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn fulfill_transfer_request(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(_dto): Json<ApproveInventoryActionDto>,
) -> ApiResult<StockTransferRequest> {
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    ok(state.inventory.fulfill_transfer(id, user_id).await?)
}

#[utoipa::path(
    post,
    path = "/inventory/waste-events",
    request_body = CreateStockWasteEventDto,
    responses(
        (status = 201, description = "Create waste event", body = StockWasteWithLinesEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn create_waste_event(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateStockWasteEventDto>,
) -> ApiResult<StockWasteEventWithLines> {
    let user_id = Uuid::parse_str(&claims.userId).ok();
    created(state.inventory.create_waste_event(dto, user_id).await?)
}

#[utoipa::path(
    get,
    path = "/inventory/waste-events",
    responses(
        (status = 200, description = "List waste events", body = StockWastePaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn list_waste_events(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<StockWasteFilterDto>,
) -> ApiResult<PaginationResult<StockWasteEvent>> {
    ok(state.inventory.list_waste_events(filters).await?)
}

#[utoipa::path(
    get,
    path = "/inventory/waste-events/{id}",
    params(("id" = Uuid, Path, description = "Waste event ID")),
    responses(
        (status = 200, description = "Get waste event", body = StockWasteWithLinesEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn get_waste_event(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StockWasteEventWithLines> {
    ok(state.inventory.get_waste_event(id).await?)
}

#[utoipa::path(
    patch,
    path = "/inventory/waste-events/{id}/approve",
    params(("id" = Uuid, Path, description = "Waste event ID")),
    request_body = ApproveInventoryActionDto,
    responses(
        (status = 200, description = "Approve waste event", body = StockWasteEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 409, description = "Insufficient stock", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn approve_waste_event(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(_dto): Json<ApproveInventoryActionDto>,
) -> ApiResult<StockWasteEvent> {
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    ok(state.inventory.approve_waste(id, user_id).await?)
}

#[utoipa::path(
    patch,
    path = "/inventory/waste-events/{id}/reject",
    params(("id" = Uuid, Path, description = "Waste event ID")),
    request_body = RejectStockWasteDto,
    responses(
        (status = 200, description = "Reject waste event", body = StockWasteEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn reject_waste_event(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<RejectStockWasteDto>,
) -> ApiResult<StockWasteEvent> {
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    ok(
        state
            .inventory
            .reject_waste(id, &dto.rejection_reason, user_id)
            .await?,
    )
}

#[utoipa::path(
    get,
    path = "/inventory/waste/summary",
    responses(
        (status = 200, description = "Waste summary report", body = StockWasteSummaryListEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn waste_summary(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<WasteSummaryFilterDto>,
) -> ApiResult<Vec<WasteSummaryRow>> {
    ok(state.inventory.waste_summary(filters).await?)
}

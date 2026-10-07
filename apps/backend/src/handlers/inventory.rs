use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{
    created, ok, ApiResult, ApproveInventoryActionDto, PaginationResult, ReceiptSummaryFilterDto,
    WasteSummaryFilterDto,
};
use crate::error::AppError;
use crate::middleware::{AdminOrStaff, AdminUser};
use crate::models::{
    CreateInventoryLocationDto, CreateStockAdjustmentDto, CreateStockReceiptDto,
    CreateStockTransferRequestDto, CreateStockWasteEventDto, InventoryLocation,
    InventoryLocationFilterDto, LocationStockFilterDto, LocationStockRow, ReceiptSummaryRow,
    RejectStockTransferDto, RejectStockWasteDto, StockAdjustment, StockAdjustmentFilterDto,
    StockAdjustmentWithLines, StockReceipt, StockReceiptFilterDto, StockReceiptWithLines,
    StockTransferFilterDto, StockTransferRequest, StockTransferRequestWithLines, StockWasteEvent,
    StockWasteEventWithLines, StockWasteFilterDto, UpdateInventoryLocationDto, WasteSummaryRow,
};
use crate::openapi::responses::{
    ErrorEnvelope, InventoryLocationEnvelope, InventoryLocationPaginationEnvelope,
    LocationStockPaginationEnvelope, StockAdjustmentPaginationEnvelope,
    StockAdjustmentWithLinesEnvelope, StockReceiptPaginationEnvelope,
    StockReceiptSummaryListEnvelope, StockReceiptWithLinesEnvelope, StockTransferEnvelope,
    StockTransferPaginationEnvelope, StockTransferWithLinesEnvelope, StockWasteEnvelope,
    StockWastePaginationEnvelope, StockWasteSummaryListEnvelope, StockWasteWithLinesEnvelope,
};
use crate::repositories::{TenantInventoryRepository, TenantSettingsRepository};

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
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<InventoryLocationFilterDto>,
) -> ApiResult<PaginationResult<InventoryLocation>> {
    let repo = scoped_inventory(&state, &claims, "inventory:read").await?;
    if let Some(location_id) = filters.venue_location_id {
        let org = Uuid::parse_str(&claims.tenantId)
            .map_err(|_| AppError::Forbidden("Select an organization".into()))?;
        let user = claims
            .user_id_uuid()
            .ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))?;
        TenantSettingsRepository::new(state.business_db(&claims).await?)
            .ensure_location_permission(org, location_id, user, "inventory:read")
            .await?;
    }
    ok(repo.list_locations(&filters).await?)
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
    let repo = scoped_inventory(&state, &claims, "inventory:manage").await?;
    let user_id = Uuid::parse_str(&claims.userId).ok();
    TenantSettingsRepository::new(state.business_db(&claims).await?)
        .ensure_location_permission(
            Uuid::parse_str(&claims.tenantId)
                .map_err(|_| AppError::Forbidden("Select an organization".into()))?,
            dto.venue_location_id
                .ok_or_else(|| AppError::bad_request_code("LOCATION_REQUIRED", None))?,
            user_id.ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))?,
            "inventory:manage",
        )
        .await?;
    created(repo.create_location(&dto, user_id).await?)
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
    let repo = scoped_inventory(&state, &claims, "inventory:manage").await?;
    let user_id = Uuid::parse_str(&claims.userId).ok();
    let existing = repo.get_location(id).await?;
    let org = Uuid::parse_str(&claims.tenantId)
        .map_err(|_| AppError::Forbidden("Select an organization".into()))?;
    let user = user_id.ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))?;
    TenantSettingsRepository::new(state.business_db(&claims).await?)
        .ensure_location_permission(org, existing.venue_location_id, user, "inventory:manage")
        .await?;
    if let Some(location_id) = dto.venue_location_id {
        TenantSettingsRepository::new(state.business_db(&claims).await?)
            .ensure_location_permission(org, location_id, user, "inventory:manage")
            .await?;
    }
    ok(repo.update_location(id, &dto, user_id).await?)
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
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(mut filters): Query<LocationStockFilterDto>,
) -> ApiResult<PaginationResult<LocationStockRow>> {
    let repo = scoped_inventory(&state, &claims, "inventory:read").await?;
    let org = Uuid::parse_str(&claims.tenantId)
        .map_err(|_| AppError::Forbidden("Select an organization".into()))?;
    let user = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))?;
    if let Some(id) = filters.location_id {
        let location = repo.get_location(id).await?;
        if filters
            .venue_location_id
            .is_some_and(|requested| requested != location.venue_location_id)
        {
            return Err(AppError::BadRequest(
                "Inventory location is outside the selected venue".into(),
            ));
        }
        filters.venue_location_id = Some(location.venue_location_id);
    }
    if let Some(location_id) = filters.venue_location_id {
        TenantSettingsRepository::new(state.business_db(&claims).await?)
            .ensure_location_permission(org, location_id, user, "inventory:read")
            .await?;
    }
    ok(repo.list_stock(&filters).await?)
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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateStockReceiptDto>,
) -> ApiResult<StockReceiptWithLines> {
    let repo = scoped_inventory(&state, &claims, "inventory:manage").await?;
    let user_id = Uuid::parse_str(&claims.userId).ok();
    let (receipt, lines) = repo.create_receipt(&dto, user_id).await?;
    created(StockReceiptWithLines { receipt, lines })
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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<StockReceiptFilterDto>,
) -> ApiResult<PaginationResult<StockReceipt>> {
    let repo = scoped_inventory(&state, &claims, "inventory:read").await?;
    ok(repo.list_receipts(&filters).await?)
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
    let repo = scoped_inventory(&state, &claims, "inventory:manage").await?;
    let user_id = Uuid::parse_str(&claims.userId).ok();
    let (adjustment, lines) = repo.create_adjustment(&dto, user_id).await?;
    created(StockAdjustmentWithLines { adjustment, lines })
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
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<StockAdjustmentFilterDto>,
) -> ApiResult<PaginationResult<StockAdjustment>> {
    let repo = scoped_inventory(&state, &claims, "inventory:read").await?;
    ok(repo.list_adjustments(&filters).await?)
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
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StockAdjustmentWithLines> {
    let repo = scoped_inventory(&state, &claims, "inventory:read").await?;
    ok(repo.get_adjustment(id).await?)
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
    let repo = scoped_inventory(&state, &claims, "inventory:transfer_request").await?;
    let user_id = Uuid::parse_str(&claims.userId).ok();
    created(repo.request_transfer(dto, user_id).await?)
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
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<StockTransferFilterDto>,
) -> ApiResult<PaginationResult<StockTransferRequest>> {
    let repo = scoped_inventory(&state, &claims, "inventory:read").await?;
    ok(repo.list_transfer_requests(&filters).await?)
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
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StockTransferRequestWithLines> {
    let repo = scoped_inventory(&state, &claims, "inventory:read").await?;
    ok(repo.get_transfer_request(id).await?)
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
    let repo = scoped_inventory(&state, &claims, "inventory:transfer_fulfill").await?;
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    ok(repo.approve_transfer(id, user_id).await?)
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
    let repo = scoped_inventory(&state, &claims, "inventory:transfer_fulfill").await?;
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    ok(repo
        .reject_transfer(id, &dto.rejection_reason, user_id)
        .await?)
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
    let repo = scoped_inventory(&state, &claims, "inventory:transfer_fulfill").await?;
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    ok(repo.fulfill_transfer(id, user_id).await?)
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
    let repo = scoped_inventory(&state, &claims, "inventory:waste_record").await?;
    let user_id = Uuid::parse_str(&claims.userId).ok();
    let (event, lines) = repo.create_waste_event(&dto, user_id).await?;
    created(StockWasteEventWithLines { event, lines })
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
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<StockWasteFilterDto>,
) -> ApiResult<PaginationResult<StockWasteEvent>> {
    let repo = scoped_inventory(&state, &claims, "inventory:read").await?;
    ok(repo.list_waste_events(&filters).await?)
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
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StockWasteEventWithLines> {
    let repo = scoped_inventory(&state, &claims, "inventory:read").await?;
    ok(repo.get_waste_event(id).await?)
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
    let repo = scoped_inventory(&state, &claims, "inventory:waste_approve").await?;
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    ok(repo.approve_waste(id, user_id).await?)
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
    let repo = scoped_inventory(&state, &claims, "inventory:waste_approve").await?;
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    ok(repo
        .reject_waste(id, &dto.rejection_reason, user_id)
        .await?)
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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<WasteSummaryFilterDto>,
) -> ApiResult<Vec<WasteSummaryRow>> {
    let _ = scoped_inventory(&state, &claims, "inventory:read").await?;
    let _ = filters;
    Err(AppError::Api {
        code: "ANALYTICS_UNAVAILABLE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: None,
    })
}

#[utoipa::path(
    get,
    path = "/inventory/receipts/summary",
    params(ReceiptSummaryFilterDto),
    responses(
        (status = 200, description = "Stock receipt summary report", body = StockReceiptSummaryListEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "inventory"
)]
pub async fn receipt_summary(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<ReceiptSummaryFilterDto>,
) -> ApiResult<Vec<ReceiptSummaryRow>> {
    let _ = scoped_inventory(&state, &claims, "inventory:read").await?;
    let _ = filters;
    Err(AppError::Api {
        code: "ANALYTICS_UNAVAILABLE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: None,
    })
}

async fn scoped_inventory(
    state: &AppState,
    claims: &crate::dto::JwtUserClaims,
    permission: &str,
) -> Result<TenantInventoryRepository, AppError> {
    let db = state.business_db(claims).await?;
    let scope =
        crate::access::scope::LocationScope::resolve_tenant(db.clone(), claims, permission, None)
            .await?;
    Ok(TenantInventoryRepository::scoped(db, scope.locations))
}

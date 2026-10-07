use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::access::scope::{requested_location, LocationScope};
use crate::app::AppState;
use crate::dto::{created, ok, ApiResult, PaginationResult};
use crate::middleware::{AdminOrStaff, AdminUser, StaffUser};
use crate::models::{
    CashRegister, CashRegisterEntry, CashRegisterFilterDto, CashRegisterWithEntries,
    CloseCashRegisterDto, CreateCashRegisterEntryDto, OpenCashRegisterDto,
    ReconcileCashRegisterDto, UpdateOpeningBalanceDto,
};
use crate::openapi::responses::{
    CashRegisterEntryEnvelope, CashRegisterEnvelope, CashRegisterPaginationEnvelope,
    CashRegisterWithEntriesEnvelope, ErrorEnvelope, ExpectedClosingEnvelope,
};
use crate::repositories::{TenantCashRegisterRepository, TenantShiftRepository};

#[utoipa::path(
    post,
    path = "/cash-registers/open",
    request_body = OpenCashRegisterDto,
    responses(
        (status = 201, description = "Cash register opened", body = CashRegisterEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 409, description = "Conflict", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "cash-registers"
)]
pub async fn open_cash_register(
    State(state): State<Arc<AppState>>,
    StaffUser(claims): StaffUser,
    Json(dto): Json<OpenCashRegisterDto>,
) -> ApiResult<CashRegister> {
    let db = state.business_db(&claims).await?;
    let venue = TenantShiftRepository::new(db.clone())
        .location_id(dto.shift_id)
        .await?;
    LocationScope::resolve_tenant(db.clone(), &claims, "cash-registers:write", Some(venue)).await?;
    let actor_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::Internal("Invalid user ID in token".to_string()))?;
    let register = TenantCashRegisterRepository::new(db.clone())
        .open_register(&dto, actor_id)
        .await?;
    created(register)
}

#[utoipa::path(
    patch,
    path = "/cash-registers/{id}/close",
    params(
        ("id" = Uuid, Path, description = "Cash register ID"),
    ),
    request_body = CloseCashRegisterDto,
    responses(
        (status = 200, description = "Cash register closed", body = CashRegisterEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "cash-registers"
)]
pub async fn close_cash_register(
    State(state): State<Arc<AppState>>,
    StaffUser(claims): StaffUser,
    Path(id): Path<Uuid>,
    Json(dto): Json<CloseCashRegisterDto>,
) -> ApiResult<CashRegister> {
    let db = state.business_db(&claims).await?;
    let venue = TenantCashRegisterRepository::new(db.clone())
        .location_id(id)
        .await?;
    LocationScope::resolve_tenant(db.clone(), &claims, "cash-registers:write", Some(venue)).await?;
    let actor_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::Internal("Invalid user ID in token".to_string()))?;
    let register = TenantCashRegisterRepository::new(db.clone())
        .close_register(id, &dto, actor_id)
        .await?;
    ok(register)
}

#[utoipa::path(
    patch,
    path = "/cash-registers/{id}/reconcile",
    params(
        ("id" = Uuid, Path, description = "Cash register ID"),
    ),
    request_body = ReconcileCashRegisterDto,
    responses(
        (status = 200, description = "Cash register reconciled", body = CashRegisterEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "cash-registers"
)]
pub async fn reconcile_cash_register(
    State(state): State<Arc<AppState>>,
    AdminUser(claims): AdminUser,
    Path(id): Path<Uuid>,
    Json(dto): Json<ReconcileCashRegisterDto>,
) -> ApiResult<CashRegister> {
    let db = state.business_db(&claims).await?;
    let venue = TenantCashRegisterRepository::new(db.clone())
        .location_id(id)
        .await?;
    LocationScope::resolve_tenant(db.clone(), &claims, "cash-registers:reconcile", Some(venue))
        .await?;
    let actor_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::Internal("Invalid user ID in token".to_string()))?;
    let register = TenantCashRegisterRepository::new(db.clone())
        .reconcile(id, dto.reconciliation_notes, actor_id)
        .await?;
    ok(register)
}

#[utoipa::path(
    patch,
    path = "/cash-registers/{id}/update-opening",
    params(
        ("id" = Uuid, Path, description = "Cash register ID"),
    ),
    request_body = UpdateOpeningBalanceDto,
    responses(
        (status = 200, description = "Opening balance updated", body = CashRegisterEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "cash-registers"
)]
pub async fn update_opening_balance(
    State(state): State<Arc<AppState>>,
    AdminUser(claims): AdminUser,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateOpeningBalanceDto>,
) -> ApiResult<CashRegister> {
    let db = state.business_db(&claims).await?;
    let venue = TenantCashRegisterRepository::new(db.clone())
        .location_id(id)
        .await?;
    LocationScope::resolve_tenant(
        db.clone(),
        &claims,
        "cash-registers:adjust_opening",
        Some(venue),
    )
    .await?;
    let actor_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::Internal("Invalid user ID in token".to_string()))?;
    let register = TenantCashRegisterRepository::new(db.clone())
        .update_opening_balance(id, dto.opening_balance, dto.opening_denominations, actor_id)
        .await?;
    ok(register)
}

#[utoipa::path(
    post,
    path = "/cash-registers/{id}/entries",
    params(
        ("id" = Uuid, Path, description = "Cash register ID"),
    ),
    request_body = CreateCashRegisterEntryDto,
    responses(
        (status = 201, description = "Entry added", body = CashRegisterEntryEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "cash-registers"
)]
pub async fn add_entry(
    State(state): State<Arc<AppState>>,
    StaffUser(claims): StaffUser,
    Path(id): Path<Uuid>,
    Json(dto): Json<CreateCashRegisterEntryDto>,
) -> ApiResult<CashRegisterEntry> {
    let db = state.business_db(&claims).await?;
    let venue = TenantCashRegisterRepository::new(db.clone())
        .location_id(id)
        .await?;
    LocationScope::resolve_tenant(db.clone(), &claims, "cash-registers:write", Some(venue)).await?;
    let actor_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::Internal("Invalid user ID in token".to_string()))?;
    let entry = TenantCashRegisterRepository::new(db.clone())
        .add_entry(id, &dto, actor_id)
        .await?;
    created(entry)
}

#[utoipa::path(
    get,
    path = "/cash-registers/{id}",
    params(
        ("id" = Uuid, Path, description = "Cash register ID"),
    ),
    responses(
        (status = 200, description = "Cash register details", body = CashRegisterWithEntriesEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "cash-registers"
)]
pub async fn get_cash_register(
    State(state): State<Arc<AppState>>,
    AdminOrStaff(claims): AdminOrStaff,
    Path(id): Path<Uuid>,
) -> ApiResult<CashRegisterWithEntries> {
    let db = state.business_db(&claims).await?;
    let venue = TenantCashRegisterRepository::new(db.clone())
        .location_id(id)
        .await?;
    LocationScope::resolve_tenant(db.clone(), &claims, "cash-registers:read", Some(venue)).await?;
    let register = TenantCashRegisterRepository::new(db.clone())
        .get_by_id(id)
        .await?;
    ok(register)
}

#[utoipa::path(
    get,
    path = "/cash-registers",
    params(CashRegisterFilterDto),
    responses(
        (status = 200, description = "List cash registers", body = CashRegisterPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "cash-registers"
)]
pub async fn list_cash_registers(
    State(state): State<Arc<AppState>>,
    AdminOrStaff(claims): AdminOrStaff,
    headers: HeaderMap,
    Query(filters): Query<CashRegisterFilterDto>,
) -> ApiResult<PaginationResult<CashRegister>> {
    let db = state.business_db(&claims).await?;
    let scope = LocationScope::resolve_tenant(
        db.clone(),
        &claims,
        "cash-registers:read",
        requested_location(&headers)?,
    )
    .await?;
    let result = TenantCashRegisterRepository::new(db)
        .list_scoped(&filters, &scope.locations)
        .await?;
    ok(result)
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedClosingResponse {
    pub register_id: Option<uuid::Uuid>,
    pub expected_closing: f64,
    pub opening_balance: f64,
}

#[utoipa::path(
    get,
    path = "/cash-registers/active/expected-closing",
    responses(
        (status = 200, description = "Expected closing for active shift register", body = ExpectedClosingEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "cash-registers"
)]
pub async fn get_active_expected_closing(
    State(state): State<Arc<AppState>>,
    StaffUser(claims): StaffUser,
) -> ApiResult<ExpectedClosingResponse> {
    let user_id: uuid::Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID in token".to_string()))?;

    let active_shift = TenantShiftRepository::new(state.business_db(&claims).await?)
        .find_active_by_user(user_id)
        .await?
        .ok_or_else(|| {
            crate::error::AppError::NotFound("No active shift found for current user".to_string())
        })?;

    let db = state.business_db(&claims).await?;
    let venue = TenantShiftRepository::new(db.clone())
        .location_id(active_shift.id)
        .await?;
    LocationScope::resolve_tenant(db.clone(), &claims, "cash-registers:read", Some(venue)).await?;

    let register = TenantCashRegisterRepository::new(db.clone())
        .get_by_shift(active_shift.id)
        .await?;

    let expected = TenantCashRegisterRepository::new(db.clone())
        .get_expected_closing(register.register.id)
        .await?;

    ok(ExpectedClosingResponse {
        register_id: Some(register.register.id),
        expected_closing: expected,
        opening_balance: register.register.opening_balance,
    })
}

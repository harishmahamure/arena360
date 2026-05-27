use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult, PaginationResult};
use crate::middleware::AdminUser;
use crate::models::{CreateVendorDto, UpdateVendorDto, Vendor, VendorFilterDto};
use crate::openapi::responses::{ErrorEnvelope, VendorEnvelope, VendorPaginationEnvelope};

#[utoipa::path(
    get,
    path = "/vendors",
    responses(
        (status = 200, description = "List vendors", body = VendorPaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "vendors"
)]
pub async fn list_vendors(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<VendorFilterDto>,
) -> ApiResult<PaginationResult<Vendor>> {
    let result = state.vendors.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/vendors/{id}",
    params(
        ("id" = Uuid, Path, description = "Vendor ID"),
    ),
    responses(
        (status = 200, description = "Get vendor", body = VendorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "vendors"
)]
pub async fn get_vendor(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Vendor> {
    let vendor = state.vendors.get_by_id(id).await?;
    ok(vendor)
}

#[utoipa::path(
    post,
    path = "/vendors",
    request_body = CreateVendorDto,
    responses(
        (status = 201, description = "Create vendor", body = VendorEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 409, description = "Conflict", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "vendors"
)]
pub async fn create_vendor(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateVendorDto>,
) -> ApiResult<Vendor> {
    let user_id = Uuid::parse_str(&claims.userId).ok();
    let vendor = state.vendors.create(dto, user_id).await?;
    created(vendor)
}

#[utoipa::path(
    patch,
    path = "/vendors/{id}",
    params(
        ("id" = Uuid, Path, description = "Vendor ID"),
    ),
    request_body = UpdateVendorDto,
    responses(
        (status = 200, description = "Update vendor", body = VendorEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 409, description = "Conflict", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "vendors"
)]
pub async fn update_vendor(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateVendorDto>,
) -> ApiResult<Vendor> {
    let user_id = Uuid::parse_str(&claims.userId).ok();
    let vendor = state.vendors.update(id, dto, user_id).await?;
    ok(vendor)
}

#[utoipa::path(
    delete,
    path = "/vendors/{id}",
    params(
        ("id" = Uuid, Path, description = "Vendor ID"),
    ),
    responses(
        (status = 200, description = "Delete vendor", body = serde_json::Value),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "vendors"
)]
pub async fn delete_vendor(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<serde_json::Value> {
    state.vendors.delete(id).await?;
    ok(serde_json::json!({"deleted": true}))
}

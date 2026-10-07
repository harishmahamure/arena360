use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::access::scope::{requested_location, LocationScope};
use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::middleware::{require_staff_for_counter, AdminOrStaff, PlayerUser};
use crate::models::{
    ConvertKioskOrderDto, CreateKioskOrderDto, KioskMenuProduct, KioskOrderFilterDto,
    KioskOrderWithItems, Transaction, UpdateKioskOrderDto,
};
use crate::openapi::responses::ErrorEnvelope;
use crate::repositories::{TenantKioskOrderRepository, TenantShiftRepository};

#[utoipa::path(
    get,
    operation_id = "kiosk_list_products",
    path = "/kiosk/products",
    responses(
        (status = 200, description = "Kiosk product menu"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "kiosk"
)]
pub async fn list_products(
    player: PlayerUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Vec<KioskMenuProduct>> {
    let products = state
        .kiosk_orders
        .list_menu_tenant(
            state.business_db(&player.0).await?,
            player.device_id()?,
            &state.products,
        )
        .await?;
    ok(products)
}

#[utoipa::path(
    post,
    path = "/kiosk/orders",
    request_body = CreateKioskOrderDto,
    responses(
        (status = 201, description = "Order placed"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "No active session", body = ErrorEnvelope),
        (status = 409, description = "Open order already exists", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "kiosk"
)]
pub async fn place_order(
    player: PlayerUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateKioskOrderDto>,
) -> ApiResult<KioskOrderWithItems> {
    let player_id = player.player_id()?;
    let device_id = player.device_id()?;
    let order = state
        .kiosk_orders
        .place_order_tenant(
            state.business_db(&player.0).await?,
            player_id,
            device_id,
            dto,
        )
        .await?;
    created(order)
}

#[utoipa::path(
    get,
    path = "/kiosk/orders/current",
    responses(
        (status = 200, description = "Current open order or null"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "kiosk"
)]
pub async fn current_order(
    player: PlayerUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Option<KioskOrderWithItems>> {
    let player_id = player.player_id()?;
    let device_id = player.device_id()?;
    let order = state
        .kiosk_orders
        .current_order_for_player_tenant(state.business_db(&player.0).await?, player_id, device_id)
        .await?;
    ok(order)
}

#[utoipa::path(
    get,
    path = "/kiosk-orders",
    responses(
        (status = 200, description = "List kiosk orders"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "kiosk-orders"
)]
pub async fn list_orders(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(filters): Query<KioskOrderFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<KioskOrderWithItems>> {
    let db = state.business_db(&claims).await?;
    let scope = LocationScope::resolve_tenant(
        db.clone(),
        &claims,
        "transactions:read",
        requested_location(&headers)?,
    )
    .await?;
    ok(TenantKioskOrderRepository::new(db)
        .list_scoped(&filters, Some(&scope.locations))
        .await?)
}

#[utoipa::path(
    get,
    path = "/kiosk-orders/{id}",
    params(("id" = Uuid, Path)),
    responses(
        (status = 200, description = "Order detail"),
        (status = 404, description = "Not found", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "kiosk-orders"
)]
pub async fn get_order(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<KioskOrderWithItems> {
    let db = state.business_db(&claims).await?;
    let venue = TenantKioskOrderRepository::new(db.clone())
        .venue_for_order(id)
        .await?;
    LocationScope::resolve_tenant(db.clone(), &claims, "transactions:read", Some(venue)).await?;
    let order = state
        .kiosk_orders
        .get_by_id_tenant(state.business_db(&claims).await?, id)
        .await?;
    ok(order)
}

#[utoipa::path(
    patch,
    path = "/kiosk-orders/{id}",
    params(("id" = Uuid, Path)),
    request_body = UpdateKioskOrderDto,
    responses(
        (status = 200, description = "Order updated"),
        (status = 404, description = "Not found", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "kiosk-orders"
)]
pub async fn update_order(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateKioskOrderDto>,
) -> ApiResult<KioskOrderWithItems> {
    let db = state.business_db(&claims).await?;
    let venue = TenantKioskOrderRepository::new(db.clone())
        .venue_for_order(id)
        .await?;
    LocationScope::resolve_tenant(db.clone(), &claims, "transactions:write", Some(venue)).await?;
    require_staff_for_counter(&claims)?;
    let user_id = claims.user_id_uuid().ok_or_else(|| {
        crate::error::AppError::BadRequest("Invalid user ID in token".to_string())
    })?;
    let shift = TenantShiftRepository::new(db.clone())
        .find_active_by_user(user_id)
        .await?
        .ok_or_else(|| {
            crate::error::AppError::BadRequest("No active shift found for current user".to_string())
        })?;
    if TenantShiftRepository::new(db).location_id(shift.id).await? != venue {
        return Err(crate::error::AppError::Forbidden(
            "Active shift must match the order venue".into(),
        ));
    }
    ok(
        TenantKioskOrderRepository::new(state.business_db(&claims).await?)
            .update_status_for_shift(id, &dto.status, shift.id, user_id)
            .await?,
    )
}

#[utoipa::path(
    post,
    path = "/kiosk-orders/{id}/convert",
    params(("id" = Uuid, Path)),
    request_body = ConvertKioskOrderDto,
    responses(
        (status = 201, description = "Order converted to sale"),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "kiosk-orders"
)]
pub async fn convert_order(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<ConvertKioskOrderDto>,
) -> ApiResult<Transaction> {
    let db = state.business_db(&claims).await?;
    let venue = TenantKioskOrderRepository::new(db.clone())
        .venue_for_order(id)
        .await?;
    LocationScope::resolve_tenant(db.clone(), &claims, "transactions:write", Some(venue)).await?;
    let user_id = claims.user_id_uuid().ok_or_else(|| {
        crate::error::AppError::BadRequest("Invalid user ID in token".to_string())
    })?;

    require_staff_for_counter(&claims)?;

    let active_shift =
        crate::repositories::TenantShiftRepository::new(state.business_db(&claims).await?)
            .find_active_by_user(user_id)
            .await?
            .ok_or_else(|| {
                crate::error::AppError::BadRequest(
                    "No active shift found for current user".to_string(),
                )
            })?;

    let mut convert = dto;
    if convert.payment_status.is_none() {
        convert.payment_status = Some("completed".to_string());
    }

    let tx = state
        .kiosk_orders
        .convert_to_sale_tenant(
            state.business_db(&claims).await?,
            id,
            convert,
            active_shift.id,
            user_id,
            &state.transactions,
        )
        .await?;

    created(tx)
}

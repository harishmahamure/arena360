use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::middleware::{require_staff_for_counter, AdminOrStaff, PlayerUser};
use crate::models::{
    ConvertKioskOrderDto, CreateKioskOrderDto, KioskMenuProduct, KioskOrderFilterDto,
    KioskOrderWithItems, Transaction, UpdateKioskOrderDto,
};
use crate::openapi::responses::ErrorEnvelope;

#[utoipa::path(
    get,
    path = "/kiosk/products",
    responses(
        (status = 200, description = "Kiosk product menu"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "kiosk"
)]
pub async fn list_products(
    _player: PlayerUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Vec<KioskMenuProduct>> {
    let products = state.kiosk_orders.list_menu().await?;
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
        .place_order(player_id, device_id, dto)
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
        .current_order_for_player(player_id, device_id)
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
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<KioskOrderFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<KioskOrderWithItems>> {
    let result = state.kiosk_orders.list(filters).await?;
    ok(result)
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
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<KioskOrderWithItems> {
    let order = state.kiosk_orders.get_by_id(id).await?;
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
    require_staff_for_counter(&claims)?;
    let order = state.kiosk_orders.update_status(id, &dto.status).await?;
    ok(order)
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
    let user_id = claims.user_id_uuid().ok_or_else(|| {
        crate::error::AppError::BadRequest("Invalid user ID in token".to_string())
    })?;

    require_staff_for_counter(&claims)?;

    let active_shift = state.shifts.get_active(user_id).await?.ok_or_else(|| {
        crate::error::AppError::BadRequest("No active shift found for current user".to_string())
    })?;

    let mut convert = dto;
    if convert.payment_status.is_none() {
        convert.payment_status = Some("completed".to_string());
    }

    let actor_role = claims.roles.first().map(|s| s.as_str());
    let tx = state
        .kiosk_orders
        .convert_to_sale(
            id,
            convert,
            active_shift.id,
            user_id,
            actor_role,
            &state.transactions,
            &state.cash_registers,
        )
        .await?;

    created(tx)
}

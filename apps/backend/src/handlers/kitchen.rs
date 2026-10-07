use crate::{
    app::AppState,
    dto::{ok, ApiResult},
    error::AppError,
    middleware::{AdminOrStaff, AdminUser},
    services::TenantKitchenService,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct TicketQuery {
    pub history: Option<bool>,
}

pub async fn list(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(query): Query<TicketQuery>,
) -> ApiResult<Value> {
    ok(TenantKitchenService::new(state.business_db(&claims).await?)
        .list(query.history.unwrap_or(false))
        .await?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdvanceTicket {
    pub status: String,
    pub expected_revision: i32,
    pub reason: Option<String>,
}

pub async fn advance(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<AdvanceTicket>,
) -> ApiResult<Value> {
    ok(TenantKitchenService::new(state.business_db(&claims).await?)
        .advance(
            id,
            &dto.status,
            dto.expected_revision,
            dto.reason.as_deref(),
            claims
                .user_id_uuid()
                .ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))?,
        )
        .await?)
}

pub async fn menu(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Value> {
    ok(TenantKitchenService::new(state.business_db(&claims).await?)
        .menu()
        .await?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuSetting {
    pub enabled: bool,
    pub station: String,
    pub prep_minutes: i32,
    pub expected_revision: i32,
}

pub async fn save_menu(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<MenuSetting>,
) -> ApiResult<Value> {
    ok(TenantKitchenService::new(state.business_db(&claims).await?)
        .save_menu(
            id,
            dto.enabled,
            &dto.station,
            dto.prep_minutes,
            dto.expected_revision,
            claims
                .user_id_uuid()
                .ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))?,
        )
        .await?)
}

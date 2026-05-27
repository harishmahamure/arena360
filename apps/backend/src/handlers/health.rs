use axum::{extract::State, Json};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::app::AppState;
use crate::config::ping;
use crate::dto::{ok, ApiResult};
use crate::openapi::responses::{
    ErrorEnvelope, HealthEnvelope, LegacyHealthResponse, LiveHealthEnvelope,
};

#[derive(serde::Serialize, ToSchema)]
pub struct HealthData {
    pub status: &'static str,
}

#[derive(serde::Serialize, ToSchema)]
pub struct LiveHealthData {
    pub status: &'static str,
    pub timestamp: String,
    pub db: &'static str,
}

pub async fn health_check(_state: State<Arc<AppState>>) -> ApiResult<HealthData> {
    ok(HealthData { status: "ok" })
}

#[utoipa::path(
    get,
    path = "/health/live",
    responses(
        (status = 200, description = "Live health check", body = LiveHealthEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    tag = "health"
)]
pub async fn live_check(State(state): State<Arc<AppState>>) -> ApiResult<LiveHealthData> {
    let db = if ping(&state.db).await { "up" } else { "down" };
    ok(LiveHealthData {
        status: "ok",
        timestamp: chrono::Utc::now().to_rfc3339(),
        db,
    })
}

#[utoipa::path(
    get,
    path = "/health/ready",
    responses(
        (status = 200, description = "Ready health check", body = HealthEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    tag = "health"
)]
pub async fn ready_check(State(state): State<Arc<AppState>>) -> ApiResult<HealthData> {
    if ping(&state.db).await {
        ok(HealthData { status: "ok" })
    } else {
        Err(crate::error::AppError::Internal("Database unavailable".to_string()))
    }
}

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Legacy health check", body = LegacyHealthResponse),
    ),
    tag = "health"
)]
pub async fn health_check_legacy(state: State<Arc<AppState>>) -> Json<serde_json::Value> {
    let db_up = ping(&state.db).await;
    Json(serde_json::json!({
        "status": if db_up { "ok" } else { "error" },
        "info": { "database": { "status": if db_up { "up" } else { "down" } } },
        "error": {},
        "details": { "database": { "status": if db_up { "up" } else { "down" } } }
    }))
}

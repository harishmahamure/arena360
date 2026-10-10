use axum::{extract::State, Json};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::app::AppState;
use crate::cache;
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
    pub redis: &'static str,
}

#[derive(serde::Serialize, ToSchema)]
pub struct ReadyHealthData {
    pub status: &'static str,
    pub db: &'static str,
    pub redis: &'static str,
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
    let db = if storage_ready(&state).await {
        "up"
    } else {
        "down"
    };
    let redis = if cache::ping(state.cache.as_ref()).await {
        "up"
    } else {
        "degraded"
    };
    ok(LiveHealthData {
        status: "ok",
        timestamp: crate::time::utc_timestamp(&chrono::Utc::now()),
        db,
        redis,
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
pub async fn ready_check(State(state): State<Arc<AppState>>) -> ApiResult<ReadyHealthData> {
    let db_up = storage_ready(&state).await;
    if db_up {
        ok(ReadyHealthData {
            status: "ok",
            db: "up",
            redis: if cache::ping(state.cache.as_ref()).await {
                "up"
            } else {
                "degraded"
            },
        })
    } else {
        Err(crate::error::AppError::Internal(
            "Database unavailable".to_string(),
        ))
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
    let db_up = storage_ready(&state).await;
    let redis_up = cache::ping(state.cache.as_ref()).await;
    Json(serde_json::json!({
        "status": if db_up { "ok" } else { "error" },
        "info": {
            "database": { "status": if db_up { "up" } else { "down" } },
            "redis": { "status": if redis_up { "up" } else { "degraded" } }
        },
        "error": {},
        "details": {
            "database": { "status": if db_up { "up" } else { "down" } },
            "redis": { "status": if redis_up { "up" } else { "degraded" } }
        }
    }))
}

/// Readiness checks only the stores required by this process's configured roles.
/// A cell can continue serving its valid leases during a control-plane outage.
async fn storage_ready(state: &AppState) -> bool {
    if state.settings.roles.control {
        let Some(pool) = state.control_db.as_ref() else {
            return false;
        };
        if !ping(pool).await {
            return false;
        }
    }
    if state.settings.roles.router && state.routing.is_none() {
        return false;
    }
    if state.settings.roles.cell {
        let Some(manager) = state.tenant_dbs.as_ref() else {
            return false;
        };
        for db in manager.open_handles().await {
            if db.ensure_current_owner().is_err() {
                return false;
            }
            let Ok(pool) = db.background_read_pool() else {
                return false;
            };
            if sqlx::query("SELECT 1").execute(&pool).await.is_err() {
                return false;
            }
        }
    }
    true
}

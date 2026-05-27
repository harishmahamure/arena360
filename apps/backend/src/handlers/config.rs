use axum::extract::{Path, Query, State};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{ok, ApiResult};
use crate::error::AppError;
use crate::middleware::AdminUser;
use crate::models::{ConfigFilterDto, Configuration, UpsertConfigDto};
use crate::openapi::responses::{ConfigurationEnvelope, ConfigurationListEnvelope, ErrorEnvelope};

#[utoipa::path(
    get,
    path = "/config",
    params(ConfigFilterDto),
    responses(
        (status = 200, description = "List configurations", body = ConfigurationListEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "config"
)]
pub async fn list_config(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<ConfigFilterDto>,
) -> ApiResult<Vec<Configuration>> {
    let configs = state.config.list(filters).await?;
    ok(configs)
}

#[utoipa::path(
    get,
    path = "/config/{key}",
    params(
        ("key" = String, Path, description = "Configuration key (dot notation, e.g. business.name)"),
    ),
    responses(
        (status = 200, description = "Get configuration", body = ConfigurationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "config"
)]
pub async fn get_config(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> ApiResult<Configuration> {
    let config = state.config.get(&key).await?;
    ok(config)
}

#[utoipa::path(
    put,
    path = "/config/{key}",
    params(
        ("key" = String, Path, description = "Configuration key (dot notation, e.g. business.name)"),
    ),
    request_body = UpsertConfigDto,
    responses(
        (status = 200, description = "Upsert configuration", body = ConfigurationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "config"
)]
pub async fn upsert_config(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(dto): Json<UpsertConfigDto>,
) -> ApiResult<Configuration> {
    let actor_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Internal("Invalid user ID in token".to_string()))?;
    let config = state.config.upsert(&key, dto, actor_id).await?;
    ok(config)
}

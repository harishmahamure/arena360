use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{ok, ApiResult};
use crate::error::AppError;
use crate::middleware::{AdminUser, AuthUser};
use crate::models::{
    ConfigFilterDto, Configuration, ConfigurationSnapshot, ConfigurationSnapshotQuery,
    DeleteSettingOverrideQuery, EffectiveSettingsQuery, ResolvedSetting, SettingDefinition,
    SettingHistoryQuery, SettingOverride, SettingRevision, UpsertConfigDto,
    UpsertSettingOverrideDto, VenueLocation,
};
use crate::openapi::responses::{
    ConfigurationEnvelope, ConfigurationListEnvelope, ConfigurationSnapshotEnvelope, ErrorEnvelope,
    ResolvedSettingListEnvelope, SettingCatalogEnvelope, SettingOverrideEnvelope,
    SettingRevisionListEnvelope, VenueLocationListEnvelope,
};

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

fn actor_id(claims: &crate::dto::JwtUserClaims) -> Result<Uuid, AppError> {
    claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user identity".to_string()))
}

fn request_id(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
}

#[utoipa::path(
    get,
    path = "/organizations/{org_id}/locations",
    params(("org_id" = Uuid, Path)),
    responses((status = 200, body = VenueLocationListEnvelope)),
    security(("bearer_auth" = [])), tag = "settings"
)]
pub async fn venue_locations(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(org_id): Path<Uuid>,
) -> ApiResult<Vec<VenueLocation>> {
    let actor_id = actor_id(&claims)?;
    state
        .config
        .ensure_access(org_id, actor_id, "settings:read")
        .await?;
    ok(state.config.list_locations(org_id, actor_id).await?)
}

#[utoipa::path(
    get,
    path = "/organizations/{org_id}/settings/catalog",
    params(("org_id" = Uuid, Path)),
    responses(
        (status = 200, body = SettingCatalogEnvelope),
        (status = 403, body = ErrorEnvelope)
    ),
    security(("bearer_auth" = [])),
    tag = "settings"
)]
pub async fn settings_catalog(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(org_id): Path<Uuid>,
) -> ApiResult<Vec<SettingDefinition>> {
    state
        .config
        .ensure_access(org_id, actor_id(&claims)?, "settings:read")
        .await?;
    ok(state.config.catalog())
}

#[utoipa::path(
    get,
    path = "/organizations/{org_id}/settings/effective",
    params(("org_id" = Uuid, Path), EffectiveSettingsQuery),
    responses(
        (status = 200, body = ResolvedSettingListEnvelope),
        (status = 403, body = ErrorEnvelope)
    ),
    security(("bearer_auth" = [])),
    tag = "settings"
)]
pub async fn effective_settings(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(org_id): Path<Uuid>,
    Query(query): Query<EffectiveSettingsQuery>,
) -> ApiResult<Vec<ResolvedSetting>> {
    let actor_id = actor_id(&claims)?;
    state
        .config
        .ensure_access(org_id, actor_id, "settings:read")
        .await?;
    if let Some(location_id) = query.location_id {
        state
            .config
            .ensure_location_access(org_id, location_id, actor_id)
            .await?;
    }
    ok(state.config.effective(org_id, query).await?)
}

#[utoipa::path(
    put,
    path = "/organizations/{org_id}/settings/overrides/{key}",
    params(("org_id" = Uuid, Path), ("key" = String, Path)),
    request_body = UpsertSettingOverrideDto,
    responses(
        (status = 200, body = SettingOverrideEnvelope),
        (status = 400, body = ErrorEnvelope),
        (status = 409, body = ErrorEnvelope)
    ),
    security(("bearer_auth" = [])),
    tag = "settings"
)]
pub async fn upsert_setting_override(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path((org_id, key)): Path<(Uuid, String)>,
    headers: HeaderMap,
    Json(dto): Json<UpsertSettingOverrideDto>,
) -> ApiResult<SettingOverride> {
    let actor_id = actor_id(&claims)?;
    state
        .config
        .ensure_access(org_id, actor_id, "settings:write")
        .await?;
    if let Some(location_id) = dto.location_id {
        state
            .config
            .ensure_location_access(org_id, location_id, actor_id)
            .await?;
    }
    ok(state
        .config
        .upsert_override(org_id, &key, dto, actor_id, request_id(&headers))
        .await?)
}

#[utoipa::path(
    delete,
    path = "/organizations/{org_id}/settings/overrides/{key}",
    params(("org_id" = Uuid, Path), ("key" = String, Path), DeleteSettingOverrideQuery),
    responses(
        (status = 200, body = serde_json::Value),
        (status = 409, body = ErrorEnvelope)
    ),
    security(("bearer_auth" = [])),
    tag = "settings"
)]
pub async fn delete_setting_override(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path((org_id, key)): Path<(Uuid, String)>,
    Query(query): Query<DeleteSettingOverrideQuery>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    let actor_id = actor_id(&claims)?;
    state
        .config
        .ensure_access(org_id, actor_id, "settings:write")
        .await?;
    if let Some(location_id) = query.location_id {
        state
            .config
            .ensure_location_access(org_id, location_id, actor_id)
            .await?;
    }
    let deleted = state
        .config
        .delete_override(
            org_id,
            query.location_id,
            &key,
            query.expected_revision,
            &query.reason,
            actor_id,
            request_id(&headers),
        )
        .await?;
    ok(serde_json::json!({"deleted": deleted}))
}

#[utoipa::path(
    get,
    path = "/organizations/{org_id}/settings/history",
    params(("org_id" = Uuid, Path), SettingHistoryQuery),
    responses((status = 200, body = SettingRevisionListEnvelope)),
    security(("bearer_auth" = [])),
    tag = "settings"
)]
pub async fn setting_history(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(org_id): Path<Uuid>,
    Query(query): Query<SettingHistoryQuery>,
) -> ApiResult<Vec<SettingRevision>> {
    let actor_id = actor_id(&claims)?;
    state
        .config
        .ensure_access(org_id, actor_id, "settings:read")
        .await?;
    if let Some(location_id) = query.location_id {
        state
            .config
            .ensure_location_access(org_id, location_id, actor_id)
            .await?;
    }
    ok(state.config.history(org_id, query).await?)
}

#[utoipa::path(
    get,
    path = "/organizations/{org_id}/configuration-snapshot",
    params(("org_id" = Uuid, Path), ConfigurationSnapshotQuery),
    responses((status = 200, body = ConfigurationSnapshotEnvelope)),
    security(("bearer_auth" = [])),
    tag = "settings"
)]
pub async fn configuration_snapshot(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(org_id): Path<Uuid>,
    Query(query): Query<ConfigurationSnapshotQuery>,
) -> ApiResult<ConfigurationSnapshot> {
    let actor_id = actor_id(&claims)?;
    state
        .config
        .ensure_access(org_id, actor_id, "settings:read")
        .await?;
    if let Some(location_id) = query.location_id {
        state
            .config
            .ensure_location_access(org_id, location_id, actor_id)
            .await?;
    }
    let mut snapshot = state.config.snapshot(org_id, query.location_id).await?;
    if query
        .since_revision
        .is_some_and(|revision| revision >= snapshot.revision)
    {
        snapshot.settings.clear();
    }
    ok(snapshot)
}

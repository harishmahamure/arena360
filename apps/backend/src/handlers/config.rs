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
    DeleteSettingOverrideQuery, EffectiveSettingsQuery, ResolvedSetting, SaveVenueLocationDto,
    SettingDefinition, SettingHistoryQuery, SettingOverride, SettingRevision, UpsertConfigDto,
    UpsertSettingOverrideDto, VenueLocation,
};
use crate::openapi::responses::{
    BrandingEnvelope, ConfigurationEnvelope, ConfigurationListEnvelope,
    ConfigurationSnapshotEnvelope, ErrorEnvelope, ResolvedSettingListEnvelope,
    SettingCatalogEnvelope, SettingOverrideEnvelope, SettingRevisionListEnvelope,
    VenueLocationEnvelope, VenueLocationListEnvelope,
};
use crate::repositories::{TenantConfigRepository, TenantSettingsRepository};

/// Public white-label identity shown on the login screen and panel shell.
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Branding {
    pub name: String,
    pub logo_url: String,
    pub primary_color: String,
}

#[utoipa::path(
    get,
    path = "/branding",
    responses((status = 200, description = "Venue branding", body = BrandingEnvelope)),
    tag = "config"
)]
pub async fn branding(State(state): State<Arc<AppState>>) -> ApiResult<Branding> {
    let settings = state.config.catalog();
    let text = |key: &str| {
        settings
            .iter()
            .find(|setting| setting.key == key)
            .and_then(|setting| setting.default_value.as_str())
            .unwrap_or_default()
            .to_string()
    };
    ok(Branding {
        name: text("business.name"),
        logo_url: text("business.logo_url"),
        primary_color: text("branding.primary_color"),
    })
}

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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<ConfigFilterDto>,
) -> ApiResult<Vec<Configuration>> {
    let configs = TenantConfigRepository::new(state.business_db(&claims).await?)
        .find_all(&filters)
        .await?;
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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> ApiResult<Configuration> {
    let config = TenantConfigRepository::new(state.business_db(&claims).await?)
        .find_by_key(&key)
        .await?
        .ok_or_else(|| AppError::NotFound("Configuration not found".into()))?;
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
    let config = state
        .config
        .upsert_config_tenant(state.business_db(&claims).await?, &key, dto, actor_id)
        .await?;
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
    Query(query): Query<VenueLocationsQuery>,
) -> ApiResult<Vec<VenueLocation>> {
    let actor_id = actor_id(&claims)?;
    if query.include_inactive {
        TenantSettingsRepository::new(state.business_db(&claims).await?)
            .ensure_access(org_id, actor_id, "locations:manage")
            .await?;
        return ok(
            TenantSettingsRepository::new(state.business_db(&claims).await?)
                .list_managed_locations(org_id)
                .await?,
        );
    }
    TenantSettingsRepository::new(state.business_db(&claims).await?)
        .ensure_access(org_id, actor_id, "locations:read")
        .await?;
    ok(
        TenantSettingsRepository::new(state.business_db(&claims).await?)
            .list_locations(org_id, actor_id)
            .await?,
    )
}

#[derive(Debug, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VenueLocationsQuery {
    pub include_inactive: bool,
}

fn validate_venue(dto: &SaveVenueLocationDto) -> Result<(), AppError> {
    TenantSettingsRepository::validate_location_dto(dto)
}

#[utoipa::path(
    post,
    path = "/organizations/{org_id}/locations",
    params(("org_id" = Uuid, Path)),
    request_body = SaveVenueLocationDto,
    responses((status = 200, body = VenueLocationEnvelope)),
    security(("bearer_auth" = [])), tag = "settings"
)]
pub async fn create_venue_location(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(org_id): Path<Uuid>,
    Json(dto): Json<SaveVenueLocationDto>,
) -> ApiResult<VenueLocation> {
    TenantSettingsRepository::new(state.business_db(&claims).await?)
        .ensure_access(org_id, actor_id(&claims)?, "locations:manage")
        .await?;
    validate_venue(&dto)?;
    ok(
        TenantSettingsRepository::new(state.business_db(&claims).await?)
            .save_location(org_id, None, dto)
            .await?,
    )
}

#[utoipa::path(
    put,
    path = "/organizations/{org_id}/locations/{location_id}",
    params(("org_id" = Uuid, Path), ("location_id" = Uuid, Path)),
    request_body = SaveVenueLocationDto,
    responses((status = 200, body = VenueLocationEnvelope)),
    security(("bearer_auth" = [])), tag = "settings"
)]
pub async fn update_venue_location(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path((org_id, location_id)): Path<(Uuid, Uuid)>,
    Json(dto): Json<SaveVenueLocationDto>,
) -> ApiResult<VenueLocation> {
    TenantSettingsRepository::new(state.business_db(&claims).await?)
        .ensure_access(org_id, actor_id(&claims)?, "locations:manage")
        .await?;
    validate_venue(&dto)?;
    ok(
        TenantSettingsRepository::new(state.business_db(&claims).await?)
            .save_location(org_id, Some(location_id), dto)
            .await?,
    )
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
    TenantSettingsRepository::new(state.business_db(&claims).await?)
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
    TenantSettingsRepository::new(state.business_db(&claims).await?)
        .ensure_access(org_id, actor_id, "settings:read")
        .await?;
    if let Some(location_id) = query.location_id {
        TenantSettingsRepository::new(state.business_db(&claims).await?)
            .ensure_location_permission(org_id, location_id, actor_id, "settings:read")
            .await?;
    }
    ok(state
        .config
        .effective_tenant(state.business_db(&claims).await?, org_id, query)
        .await?)
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
    TenantSettingsRepository::new(state.business_db(&claims).await?)
        .ensure_access(org_id, actor_id, "settings:write")
        .await?;
    if let Some(location_id) = dto.location_id {
        TenantSettingsRepository::new(state.business_db(&claims).await?)
            .ensure_location_permission(org_id, location_id, actor_id, "settings:write")
            .await?;
    }
    if dto.location_id.is_none() {
        let membership = TenantSettingsRepository::new(state.business_db(&claims).await?)
            .membership_context(actor_id)
            .await?;
        if !membership.is_some_and(|m| m.role == "admin") {
            return Err(AppError::Forbidden(
                "Organization administrator access is required to change shared defaults".into(),
            ));
        }
    }
    ok(state
        .config
        .upsert_setting_tenant(
            state.business_db(&claims).await?,
            org_id,
            &key,
            dto,
            actor_id,
            request_id(&headers),
        )
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
    TenantSettingsRepository::new(state.business_db(&claims).await?)
        .ensure_access(org_id, actor_id, "settings:write")
        .await?;
    if let Some(location_id) = query.location_id {
        TenantSettingsRepository::new(state.business_db(&claims).await?)
            .ensure_location_permission(org_id, location_id, actor_id, "settings:write")
            .await?;
    }
    if query.location_id.is_none() {
        let membership = TenantSettingsRepository::new(state.business_db(&claims).await?)
            .membership_context(actor_id)
            .await?;
        if !membership.is_some_and(|m| m.role == "admin") {
            return Err(AppError::Forbidden(
                "Organization administrator access is required to change shared defaults".into(),
            ));
        }
    }
    let deleted = state
        .config
        .delete_setting_tenant(
            state.business_db(&claims).await?,
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
    TenantSettingsRepository::new(state.business_db(&claims).await?)
        .ensure_access(org_id, actor_id, "settings:read")
        .await?;
    if let Some(location_id) = query.location_id {
        TenantSettingsRepository::new(state.business_db(&claims).await?)
            .ensure_location_permission(org_id, location_id, actor_id, "settings:read")
            .await?;
    }
    let db = state.business_db(&claims).await?;
    let scope = crate::access::scope::LocationScope::resolve_tenant(
        db.clone(),
        &claims,
        "settings:read",
        query.location_id,
    )
    .await?;
    let mut history = TenantSettingsRepository::new(db)
        .history(org_id, &query)
        .await?;
    if !scope.organization_admin {
        history.retain(|item| {
            item.location_id
                .is_none_or(|id| scope.locations.contains(&id))
        });
    }
    ok(history)
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
    TenantSettingsRepository::new(state.business_db(&claims).await?)
        .ensure_access(org_id, actor_id, "settings:read")
        .await?;
    if let Some(location_id) = query.location_id {
        TenantSettingsRepository::new(state.business_db(&claims).await?)
            .ensure_location_permission(org_id, location_id, actor_id, "settings:read")
            .await?;
    }
    let mut snapshot = state
        .config
        .snapshot_all_tenant(state.business_db(&claims).await?, org_id, query.location_id)
        .await?;
    if query
        .since_revision
        .is_some_and(|revision| revision >= snapshot.revision)
    {
        snapshot.settings.clear();
    }
    ok(snapshot)
}

#[cfg(test)]
mod location_tests {
    use super::*;

    #[test]
    fn omitted_location_filter_lists_active_locations() {
        let query: VenueLocationsQuery = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(!query.include_inactive);
        let query: VenueLocationsQuery =
            serde_json::from_value(serde_json::json!({ "includeInactive": true })).unwrap();
        assert!(query.include_inactive);
    }

    #[test]
    fn venue_details_require_valid_slug_timezone_and_currency() {
        let mut dto = SaveVenueLocationDto {
            slug: "north-hall".into(),
            name: "North Hall".into(),
            timezone: "Asia/Kolkata".into(),
            currency: "INR".into(),
            is_active: None,
        };
        assert!(validate_venue(&dto).is_ok());
        dto.slug = "North Hall".into();
        assert!(validate_venue(&dto).is_err());
        dto.slug = "north-hall".into();
        dto.timezone = "Mars/Olympus".into();
        assert!(validate_venue(&dto).is_err());
        dto.timezone = "Asia/Kolkata".into();
        dto.currency = "ZZZ".into();
        assert!(validate_venue(&dto).is_err());
    }
}

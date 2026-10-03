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
    UpsertSettingOverrideDto, VenueLocation, SaveVenueLocationDto,
};
use crate::openapi::responses::{
    BrandingEnvelope, ConfigurationEnvelope, ConfigurationListEnvelope,
    ConfigurationSnapshotEnvelope, ErrorEnvelope, ResolvedSettingListEnvelope,
    SettingCatalogEnvelope, SettingOverrideEnvelope, SettingRevisionListEnvelope,
    VenueLocationEnvelope, VenueLocationListEnvelope,
};

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
    let settings = state
        .config
        .effective(
            crate::models::DEFAULT_ORGANIZATION_ID,
            EffectiveSettingsQuery {
                location_id: Some(crate::models::DEFAULT_VENUE_LOCATION_ID),
                category: None,
            },
        )
        .await?;
    let text = |key: &str| {
        settings
            .iter()
            .find(|setting| setting.key == key)
            .and_then(|setting| setting.value.as_str())
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
    Query(query): Query<VenueLocationsQuery>,
) -> ApiResult<Vec<VenueLocation>> {
    let actor_id = actor_id(&claims)?;
    if query.include_inactive {
        state.config.ensure_access(org_id, actor_id, "locations:manage").await?;
        return ok(state.config.list_managed_locations(org_id).await?);
    }
    state
        .config
        .ensure_access(org_id, actor_id, "locations:read")
        .await?;
    ok(state.config.list_locations(org_id, actor_id).await?)
}

#[derive(Debug, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VenueLocationsQuery { pub include_inactive: bool }

fn validate_venue(dto: &SaveVenueLocationDto) -> Result<(), AppError> {
    let slug = dto.slug.trim();
    if slug.is_empty() || slug.len() > 80 || !slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        || dto.name.trim().is_empty() || dto.name.trim().len() > 160
        || dto.timezone.trim().is_empty() || dto.timezone.len() > 80
        || dto.currency.len() != 3 || !dto.currency.chars().all(|c| c.is_ascii_uppercase()) {
        return Err(AppError::BadRequest("Use a lowercase location slug, a name, an IANA timezone, and a three-letter currency code".into()));
    }
    crate::services::settings_catalog::validate("Asia/Kolkata", "venue.timezone", &serde_json::json!(dto.timezone), true)?;
    crate::services::settings_catalog::validate("Asia/Kolkata", "pricing.currency", &serde_json::json!(dto.currency), true)?;
    Ok(())
}

const VENUE_COLUMNS: &str = r#"id, "organizationId" AS organization_id, slug, name, timezone, currency, "isActive" AS is_active"#;

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
    state.config.ensure_access(org_id, actor_id(&claims)?, "locations:manage").await?;
    validate_venue(&dto)?;
    let query = format!(r#"INSERT INTO venue_locations ("organizationId", slug, name, timezone, currency, "isActive")
        VALUES ($1,$2,$3,$4,$5,COALESCE($6,TRUE)) RETURNING {VENUE_COLUMNS}"#);
    let location = sqlx::query_as::<_, VenueLocation>(&query)
        .bind(org_id).bind(dto.slug.trim()).bind(dto.name.trim())
        .bind(dto.timezone.trim()).bind(&dto.currency).bind(dto.is_active)
        .fetch_one(&state.db).await.map_err(|error| {
            if error.as_database_error().is_some_and(|db| db.is_unique_violation()) {
                AppError::Conflict("A location with this slug already exists".into())
            } else { AppError::Database(error) }
        })?;
    ok(location)
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
    state.config.ensure_access(org_id, actor_id(&claims)?, "locations:manage").await?;
    validate_venue(&dto)?;
    if dto.is_active == Some(false) {
        let devices: i64 = sqlx::query_scalar(r#"SELECT count(*) FROM devices WHERE "organizationId"=$1 AND "locationId"=$2 AND "deletedAt" IS NULL"#)
            .bind(org_id).bind(location_id).fetch_one(&state.db).await?;
        if devices > 0 {
            return Err(AppError::Conflict("Move or retire devices before deactivating this location".into()));
        }
        let stores: i64 = sqlx::query_scalar(r#"SELECT count(*) FROM inventory_locations WHERE "venueLocationId"=$1 AND "isActive" AND "deletedAt" IS NULL"#)
            .bind(location_id).fetch_one(&state.db).await?;
        if stores > 0 {
            return Err(AppError::Conflict("Move or deactivate inventory locations before deactivating this venue".into()));
        }
        let shifts: i64 = sqlx::query_scalar(r#"SELECT count(*) FROM shifts WHERE "venueLocationId"=$1 AND status='active'"#)
            .bind(location_id).fetch_one(&state.db).await?;
        if shifts > 0 {
            return Err(AppError::Conflict("Close active shifts before deactivating this venue".into()));
        }
    }
    let query = format!(r#"UPDATE venue_locations SET slug=$3, name=$4, timezone=$5, currency=$6,
        "isActive"=COALESCE($7,"isActive"), "updatedAt"=NOW()
        WHERE "organizationId"=$1 AND id=$2 RETURNING {VENUE_COLUMNS}"#);
    let location = sqlx::query_as::<_, VenueLocation>(&query)
        .bind(org_id).bind(location_id).bind(dto.slug.trim()).bind(dto.name.trim())
        .bind(dto.timezone.trim()).bind(&dto.currency).bind(dto.is_active)
        .fetch_optional(&state.db).await.map_err(|error| {
            if error.as_database_error().is_some_and(|db| db.is_unique_violation()) {
                AppError::Conflict("A location with this slug already exists".into())
            } else { AppError::Database(error) }
        })?.ok_or_else(|| AppError::NotFound("Location not found".into()))?;
    ok(location)
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
            .ensure_location_permission(org_id, location_id, actor_id, "settings:read")
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
            .ensure_location_permission(org_id, location_id, actor_id, "settings:write")
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
            .ensure_location_permission(org_id, location_id, actor_id, "settings:write")
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
            .ensure_location_permission(org_id, location_id, actor_id, "settings:read")
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
            .ensure_location_permission(org_id, location_id, actor_id, "settings:read")
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

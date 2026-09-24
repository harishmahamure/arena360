use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, get_or_set, keys, CacheService};
use crate::error::AppError;
use crate::models::{
    ConfigFilterDto, Configuration, ConfigurationSnapshot, EffectiveSettingsQuery,
    OrganizationMembershipContext, ResolvedSetting, SettingDefinition, SettingHistoryQuery,
    SettingOverride, SettingRevision, UpsertConfigDto, UpsertSettingOverrideDto, VenueLocation,
    DEFAULT_ORGANIZATION_ID,
};
use crate::repositories::{ConfigRepository, SettingsRepository};
use crate::services::settings_catalog;

pub struct ConfigService {
    repo: ConfigRepository,
    settings_repo: SettingsRepository,
    cache: Arc<dyn CacheService>,
    default_timezone: String,
}

impl ConfigService {
    pub fn new(pool: PgPool, cache: Arc<dyn CacheService>, default_timezone: String) -> Self {
        Self {
            repo: ConfigRepository::new(pool.clone()),
            settings_repo: SettingsRepository::new(pool),
            cache,
            default_timezone,
        }
    }

    async fn invalidate_configs(&self, key: Option<&str>) -> Result<(), AppError> {
        let mut cache_keys = vec![keys::configs_all().to_string()];
        if let Some(key) = key {
            cache_keys.push(keys::config(key));
        }
        if let Err(error) = cache::invalidate(&*self.cache, &cache_keys).await {
            tracing::warn!(%error, "Legacy configuration cache invalidation failed");
        }
        Ok(())
    }

    pub async fn list(&self, filters: ConfigFilterDto) -> Result<Vec<Configuration>, AppError> {
        if filters.category.is_none() && filters.key.is_none() {
            return get_or_set(
                &*self.cache,
                keys::configs_all(),
                keys::ttl::LOOKUP,
                || async { self.repo.find_all(&filters).await },
            )
            .await;
        }
        self.repo.find_all(&filters).await
    }

    pub async fn get(&self, key: &str) -> Result<Configuration, AppError> {
        let cache_key = keys::config(key);
        get_or_set(&*self.cache, &cache_key, keys::ttl::LOOKUP, || async {
            self.repo
                .find_by_key(key)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Configuration '{key}' not found")))
        })
        .await
    }

    pub async fn upsert(
        &self,
        key: &str,
        dto: UpsertConfigDto,
        actor_id: Uuid,
    ) -> Result<Configuration, AppError> {
        let category = key.split('.').next().unwrap_or("general").to_string();
        settings_catalog::validate(&self.default_timezone, key, &dto.value, false)?;
        let config = self.repo.upsert(key, &category, &dto, actor_id).await?;
        let scoped = UpsertSettingOverrideDto {
            location_id: None,
            value: dto.value,
            reason: "Updated through legacy /config compatibility endpoint".to_string(),
            expected_revision: None,
        };
        self.settings_repo
            .upsert_override(DEFAULT_ORGANIZATION_ID, key, &scoped, actor_id, None, false)
            .await?;
        self.invalidate_scoped(DEFAULT_ORGANIZATION_ID, key, None)
            .await?;
        self.invalidate_configs(Some(key)).await?;
        Ok(config)
    }

    pub fn catalog(&self) -> Vec<SettingDefinition> {
        settings_catalog::catalog(&self.default_timezone)
    }

    pub async fn membership_context(
        &self,
        user_id: Uuid,
    ) -> Result<Option<OrganizationMembershipContext>, AppError> {
        self.settings_repo.membership_context(user_id).await
    }

    pub async fn ensure_access(
        &self,
        organization_id: Uuid,
        user_id: Uuid,
        permission: &str,
    ) -> Result<(), AppError> {
        self.settings_repo
            .ensure_access(organization_id, user_id, permission)
            .await
    }

    pub async fn list_locations(
        &self,
        organization_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<VenueLocation>, AppError> {
        self.settings_repo
            .list_locations(organization_id, user_id)
            .await
    }

    pub async fn ensure_location_access(
        &self,
        organization_id: Uuid,
        location_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        self.settings_repo
            .ensure_location_access(organization_id, location_id, user_id)
            .await
    }

    pub async fn effective(
        &self,
        organization_id: Uuid,
        query: EffectiveSettingsQuery,
    ) -> Result<Vec<ResolvedSetting>, AppError> {
        if let Some(location_id) = query.location_id {
            self.settings_repo
                .validate_location(organization_id, location_id)
                .await?;
        }
        let cache_key = keys::settings_effective(
            &organization_id,
            query.location_id.as_ref(),
            query.category.as_deref(),
        );
        get_or_set(&*self.cache, &cache_key, keys::ttl::LOOKUP, || async {
            let overrides = self
                .settings_repo
                .list_overrides(organization_id, query.location_id)
                .await?;
            Ok(self.resolve(overrides, query.location_id, query.category.as_deref()))
        })
        .await
    }

    fn resolve(
        &self,
        overrides: Vec<SettingOverride>,
        location_id: Option<Uuid>,
        category: Option<&str>,
    ) -> Vec<ResolvedSetting> {
        let mut result: std::collections::BTreeMap<String, ResolvedSetting> = self
            .catalog()
            .into_iter()
            .filter(|definition| category.is_none_or(|value| definition.category == value))
            .map(|definition| {
                let key = definition.key.clone();
                (
                    key.clone(),
                    ResolvedSetting {
                        key,
                        value: definition.default_value,
                        source_scope: "platform".to_string(),
                        source_id: None,
                        revision: 0,
                        updated_at: None,
                        overridden: false,
                    },
                )
            })
            .collect();

        for row in overrides.iter().filter(|row| row.location_id.is_none()) {
            if let Some(setting) = result.get_mut(&row.key) {
                setting.value = row.value.clone();
                setting.source_scope = "organization".to_string();
                setting.source_id = Some(row.organization_id);
                setting.revision = row.revision;
                setting.updated_at = Some(row.updated_at);
                setting.overridden = true;
            }
        }
        if let Some(location_id) = location_id {
            for row in overrides
                .iter()
                .filter(|row| row.location_id == Some(location_id))
            {
                if let Some(setting) = result.get_mut(&row.key) {
                    setting.value = row.value.clone();
                    setting.source_scope = "location".to_string();
                    setting.source_id = row.location_id;
                    setting.revision = row.revision;
                    setting.updated_at = Some(row.updated_at);
                    setting.overridden = true;
                }
            }
        }
        result.into_values().collect()
    }

    pub async fn effective_with_location(
        &self,
        organization_id: Uuid,
        query: EffectiveSettingsQuery,
    ) -> Result<Vec<ResolvedSetting>, AppError> {
        self.effective(organization_id, query).await
    }

    pub async fn resolve_value(
        &self,
        organization_id: Uuid,
        location_id: Option<Uuid>,
        key: &str,
    ) -> Result<serde_json::Value, AppError> {
        let values = self
            .effective_with_location(
                organization_id,
                EffectiveSettingsQuery {
                    location_id,
                    category: None,
                },
            )
            .await?;
        values
            .into_iter()
            .find(|setting| setting.key == key)
            .map(|setting| setting.value)
            .ok_or_else(|| AppError::NotFound(format!("Setting '{key}' not found")))
    }

    pub async fn venue_pricing_context(
        &self,
        organization_id: Uuid,
        location_id: Option<Uuid>,
    ) -> Result<(String, String, String), AppError> {
        let settings = self
            .effective(
                organization_id,
                EffectiveSettingsQuery {
                    location_id,
                    category: None,
                },
            )
            .await?;
        let string_value = |key: &str, fallback: &str| {
            settings
                .iter()
                .find(|setting| setting.key == key)
                .and_then(|setting| setting.value.as_str())
                .unwrap_or(fallback)
                .to_string()
        };
        Ok((
            string_value("venue.timezone", &self.default_timezone),
            string_value("pricing.night_window_start", "23:00"),
            string_value("pricing.night_window_end", "08:00"),
        ))
    }

    pub async fn upsert_override(
        &self,
        organization_id: Uuid,
        key: &str,
        dto: UpsertSettingOverrideDto,
        actor_id: Uuid,
        request_id: Option<&str>,
    ) -> Result<SettingOverride, AppError> {
        if dto.reason.trim().len() < 3 {
            return Err(AppError::BadRequest(
                "A change reason of at least 3 characters is required".to_string(),
            ));
        }
        settings_catalog::validate(
            &self.default_timezone,
            key,
            &dto.value,
            dto.location_id.is_some(),
        )?;
        if let Some(location_id) = dto.location_id {
            self.settings_repo
                .validate_location(organization_id, location_id)
                .await?;
        }
        let row = self
            .settings_repo
            .upsert_override(
                organization_id,
                key,
                &dto,
                actor_id,
                request_id,
                organization_id == DEFAULT_ORGANIZATION_ID && dto.location_id.is_none(),
            )
            .await?;
        self.invalidate_scoped(organization_id, key, row.location_id)
            .await?;
        Ok(row)
    }

    pub async fn delete_override(
        &self,
        organization_id: Uuid,
        location_id: Option<Uuid>,
        key: &str,
        expected_revision: Option<i64>,
        reason: &str,
        actor_id: Uuid,
        request_id: Option<&str>,
    ) -> Result<bool, AppError> {
        if reason.trim().len() < 3 {
            return Err(AppError::BadRequest(
                "A change reason of at least 3 characters is required".to_string(),
            ));
        }
        let definition = settings_catalog::find(&self.default_timezone, key)
            .ok_or_else(|| AppError::BadRequest(format!("Unknown setting key '{key}'")))?;
        let mirror_legacy_value = (organization_id == DEFAULT_ORGANIZATION_ID
            && location_id.is_none())
        .then_some(&definition.default_value);
        let deleted = self
            .settings_repo
            .delete_override(
                organization_id,
                location_id,
                key,
                expected_revision,
                reason,
                actor_id,
                request_id,
                mirror_legacy_value,
            )
            .await?;
        if deleted {
            self.invalidate_scoped(organization_id, key, location_id)
                .await?;
        }
        Ok(deleted)
    }

    pub async fn history(
        &self,
        organization_id: Uuid,
        query: SettingHistoryQuery,
    ) -> Result<Vec<SettingRevision>, AppError> {
        self.settings_repo.history(organization_id, &query).await
    }

    pub async fn snapshot(
        &self,
        organization_id: Uuid,
        location_id: Option<Uuid>,
    ) -> Result<ConfigurationSnapshot, AppError> {
        let settings = self
            .effective_with_location(
                organization_id,
                EffectiveSettingsQuery {
                    location_id,
                    category: None,
                },
            )
            .await?;
        let revision = self.settings_repo.latest_revision(organization_id).await?;
        let payload = serde_json::to_vec(&settings).map_err(|error| {
            AppError::Internal(format!("Snapshot serialization failed: {error}"))
        })?;
        use sha2::{Digest, Sha256};
        let etag = hex::encode(Sha256::digest(payload));
        Ok(ConfigurationSnapshot {
            organization_id,
            location_id,
            revision,
            etag,
            generated_at: chrono::Utc::now(),
            settings,
        })
    }

    async fn invalidate_scoped(
        &self,
        organization_id: Uuid,
        _key: &str,
        _location_id: Option<Uuid>,
    ) -> Result<(), AppError> {
        if let Err(error) = self
            .cache
            .invalidate_prefix(&keys::settings_prefix(&organization_id))
            .await
        {
            tracing::warn!(%error, %organization_id, "Settings cache invalidation failed");
        }
        Ok(())
    }
}

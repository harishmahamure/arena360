use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{keys, CacheService};
use crate::error::AppError;
use crate::models::{
    Configuration, ConfigurationSnapshot, EffectiveSettingsQuery,
    ResolvedSetting, SettingDefinition, SettingHistoryQuery,
    SettingOverride, SettingRevision, UpsertConfigDto, UpsertSettingOverrideDto,
};
use crate::repositories::TenantSettingsRepository;
use crate::services::settings_catalog;
use crate::tenancy::TenantDb;

pub struct ConfigService {
    cache: Arc<dyn CacheService>,
    default_timezone: String,
}

impl ConfigService {
    pub fn new(cache: Arc<dyn CacheService>, default_timezone: String) -> Self {
        Self { cache, default_timezone }
    }

    pub fn catalog(&self) -> Vec<SettingDefinition> {
        settings_catalog::catalog(&self.default_timezone)
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

    pub async fn upsert_config_tenant(&self,db:Arc<TenantDb>,key:&str,dto:UpsertConfigDto,actor:Uuid)->Result<Configuration,AppError>{
        settings_catalog::validate(&self.default_timezone,key,&dto.value,false)?;
        let tenant=db.tenant_id();let category=key.split('.').next().unwrap_or("general");
        let row=crate::repositories::TenantConfigRepository::new(db).upsert(key,category,&dto,actor).await?;
        self.invalidate_scoped(tenant,key,None).await?;Ok(row)
    }
    pub async fn snapshot_all_tenant(&self,db:Arc<TenantDb>,organization:Uuid,location:Option<Uuid>)->Result<ConfigurationSnapshot,AppError>{
        let timezone=db.timezone().await?;
        let (overrides,revision)=TenantSettingsRepository::new(db).snapshot_values(organization,location).await?;
        let mut values=self.resolve(overrides,location,None);
        tenant_timezone_default(&mut values,&timezone);
        snapshot_from_settings(organization,location,revision,values)
    }
    pub async fn effective_tenant(&self,db:Arc<TenantDb>,organization:Uuid,query:EffectiveSettingsQuery)->Result<Vec<ResolvedSetting>,AppError>{
        let timezone=db.timezone().await?;
        let repo=TenantSettingsRepository::new(db);
        if let Some(location)=query.location_id{repo.validate_location(organization,location).await?;}
        let overrides=repo.list_overrides(organization,query.location_id).await?;
        let mut values=self.resolve(overrides,query.location_id,query.category.as_deref());
        tenant_timezone_default(&mut values,&timezone);
        Ok(values)
    }
    pub async fn upsert_setting_tenant(&self,db:Arc<TenantDb>,organization:Uuid,key:&str,dto:UpsertSettingOverrideDto,actor:Uuid,request:Option<&str>)->Result<SettingOverride,AppError>{
        if dto.reason.trim().len()<3{return Err(AppError::BadRequest("A change reason of at least 3 characters is required".into()));}
        settings_catalog::validate(&self.default_timezone,key,&dto.value,dto.location_id.is_some())?;
        let row=TenantSettingsRepository::new(db).upsert_override(organization,key,&dto,actor,request,false).await?;
        self.invalidate_scoped(organization,key,row.location_id).await?;Ok(row)
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn delete_setting_tenant(&self,db:Arc<TenantDb>,organization:Uuid,location:Option<Uuid>,key:&str,expected:Option<i64>,reason:&str,actor:Uuid,request:Option<&str>)->Result<bool,AppError>{
        if dto_key_unknown(self,key)||reason.trim().len()<3{return Err(AppError::BadRequest("Provide a known setting and a change reason of at least 3 characters".into()));}
        let inherited=if location.is_none(){self.catalog().into_iter().find(|d|d.key==key).map(|d|d.default_value)}else{None};
        let result=TenantSettingsRepository::new(db).delete_override(organization,location,key,expected,reason,actor,request,inherited.as_ref()).await?;
        if result{self.invalidate_scoped(organization,key,location).await?;}Ok(result)
    }
    pub async fn resolve_value_tenant(
        &self, db: Arc<TenantDb>, organization_id: Uuid, location_id: Option<Uuid>, key: &str,
    ) -> Result<serde_json::Value, AppError> {
        self.effective_tenant(db, organization_id, EffectiveSettingsQuery { location_id, category: None })
            .await?.into_iter().find(|setting| setting.key == key).map(|setting| setting.value)
            .ok_or_else(|| AppError::NotFound(format!("Setting '{key}' not found")))
    }

    pub async fn effective_pricing_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        query: EffectiveSettingsQuery,
    ) -> Result<Vec<ResolvedSetting>, AppError> {
        let overrides = TenantSettingsRepository::new(db)
            .list_overrides(organization_id, query.location_id)
            .await?;
        Ok(self.resolve(overrides, query.location_id, Some("pricing")))
    }

    pub async fn resolve_pricing_value_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        location_id: Option<Uuid>,
        key: &str,
    ) -> Result<serde_json::Value, AppError> {
        require_pricing_key(key)?;
        self.effective_pricing_tenant(
            db,
            organization_id,
            EffectiveSettingsQuery {
                location_id,
                category: Some("pricing".into()),
            },
        )
        .await?
        .into_iter()
        .find(|setting| setting.key == key)
        .map(|setting| setting.value)
        .ok_or_else(|| AppError::NotFound(format!("Setting '{key}' not found")))
    }

    pub async fn venue_pricing_context_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        location_id: Option<Uuid>,
    ) -> Result<(String, String, String), AppError> {
        let settings = self
            .effective_tenant(
                db,
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

    pub async fn upsert_pricing_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        key: &str,
        dto: UpsertSettingOverrideDto,
        actor_id: Uuid,
        request_id: Option<&str>,
    ) -> Result<SettingOverride, AppError> {
        require_pricing_key(key)?;
        if dto.reason.trim().len() < 3 {
            return Err(AppError::BadRequest(
                "A change reason of at least 3 characters is required".into(),
            ));
        }
        settings_catalog::validate(
            &self.default_timezone,
            key,
            &dto.value,
            dto.location_id.is_some(),
        )?;
        let row = TenantSettingsRepository::new(db)
            .upsert_override(organization_id, key, &dto, actor_id, request_id, false)
            .await?;
        self.invalidate_scoped(organization_id, key, row.location_id)
            .await?;
        Ok(row)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn delete_pricing_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        location_id: Option<Uuid>,
        key: &str,
        expected_revision: Option<i64>,
        reason: &str,
        actor_id: Uuid,
        request_id: Option<&str>,
    ) -> Result<bool, AppError> {
        require_pricing_key(key)?;
        if reason.trim().len() < 3 {
            return Err(AppError::BadRequest(
                "A change reason of at least 3 characters is required".into(),
            ));
        }
        let deleted = TenantSettingsRepository::new(db)
            .delete_override(
                organization_id,
                location_id,
                key,
                expected_revision,
                reason,
                actor_id,
                request_id,
                None,
            )
            .await?;
        if deleted {
            self.invalidate_scoped(organization_id, key, location_id)
                .await?;
        }
        Ok(deleted)
    }

    pub async fn pricing_history_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        query: SettingHistoryQuery,
    ) -> Result<Vec<SettingRevision>, AppError> {
        if let Some(key) = query.key.as_deref() {
            require_pricing_key(key)?;
        }
        TenantSettingsRepository::new(db)
            .history(organization_id, &query)
            .await
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

fn snapshot_from_settings(
    organization_id: Uuid,
    location_id: Option<Uuid>,
    revision: i64,
    settings: Vec<ResolvedSetting>,
) -> Result<ConfigurationSnapshot, AppError> {
    let payload = serde_json::to_vec(&settings)
        .map_err(|error| AppError::Internal(format!("Snapshot serialization failed: {error}")))?;
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

fn require_pricing_key(key: &str) -> Result<(), AppError> {
    if key.starts_with("pricing.") {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "Expected a pricing.* setting".into(),
        ))
    }
}

impl ConfigService {
    pub fn tenant(db: std::sync::Arc<crate::tenancy::TenantDb>) -> crate::repositories::TenantConfigRepository { crate::repositories::TenantConfigRepository::new(db) }
}

fn dto_key_unknown(service:&ConfigService,key:&str)->bool{!service.catalog().iter().any(|d|d.key==key)}

fn tenant_timezone_default(values: &mut [ResolvedSetting], timezone: &str) {
    for value in values { if value.key == "venue.timezone" && !value.overridden { value.value = serde_json::json!(timezone); } }
}

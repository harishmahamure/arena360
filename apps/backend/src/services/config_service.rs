use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, get_or_set, keys, CacheService};
use crate::error::AppError;
use crate::models::{ConfigFilterDto, Configuration, UpsertConfigDto};
use crate::repositories::ConfigRepository;

pub struct ConfigService {
    repo: ConfigRepository,
    cache: Arc<dyn CacheService>,
}

impl ConfigService {
    pub fn new(pool: PgPool, cache: Arc<dyn CacheService>) -> Self {
        Self {
            repo: ConfigRepository::new(pool),
            cache,
        }
    }

    async fn invalidate_configs(&self, key: Option<&str>) -> Result<(), AppError> {
        let mut cache_keys = vec![keys::configs_all().to_string()];
        if let Some(key) = key {
            cache_keys.push(keys::config(key));
        }
        cache::invalidate(&*self.cache, &cache_keys).await
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
        let config = self.repo.upsert(key, &category, &dto, actor_id).await?;
        self.invalidate_configs(Some(key)).await?;
        Ok(config)
    }
}

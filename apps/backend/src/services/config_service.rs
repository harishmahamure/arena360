use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{ConfigFilterDto, Configuration, UpsertConfigDto};
use crate::repositories::ConfigRepository;

pub struct ConfigService {
    repo: ConfigRepository,
}

impl ConfigService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: ConfigRepository::new(pool),
        }
    }

    pub async fn list(&self, filters: ConfigFilterDto) -> Result<Vec<Configuration>, AppError> {
        self.repo.find_all(&filters).await
    }

    pub async fn get(&self, key: &str) -> Result<Configuration, AppError> {
        self.repo
            .find_by_key(key)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Configuration '{key}' not found")))
    }

    pub async fn upsert(
        &self,
        key: &str,
        dto: UpsertConfigDto,
        actor_id: Uuid,
    ) -> Result<Configuration, AppError> {
        let category = key.split('.').next().unwrap_or("general").to_string();

        self.repo.upsert(key, &category, &dto, actor_id).await
    }
}

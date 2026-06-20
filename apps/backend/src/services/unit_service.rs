use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, get_or_set, keys, CacheService};
use crate::error::AppError;
use crate::models::{CreateUnitDto, Unit, UnitFilterDto, UpdateUnitDto};
use crate::repositories::UnitRepository;
use crate::validation::{optional_unit_type, require_unit_type};

pub struct UnitService {
    repo: UnitRepository,
    cache: Arc<dyn CacheService>,
}

impl UnitService {
    pub fn new(pool: PgPool, cache: Arc<dyn CacheService>) -> Self {
        Self {
            repo: UnitRepository::new(pool),
            cache,
        }
    }

    async fn invalidate_units(&self, id: Option<Uuid>) -> Result<(), AppError> {
        let mut cache_keys = vec![keys::units_all().to_string()];
        if let Some(id) = id {
            cache_keys.push(keys::unit(&id));
        }
        cache::invalidate(&*self.cache, &cache_keys).await?;
        self.cache.invalidate_prefix("units:list:").await
    }

    pub async fn list(
        &self,
        filters: UnitFilterDto,
    ) -> Result<crate::dto::PaginationResult<Unit>, AppError> {
        let cache_key = keys::filter_hash(&filters);
        let cache_key = format!("units:list:{cache_key}");
        get_or_set(&*self.cache, &cache_key, keys::ttl::LOOKUP, || async {
            self.repo.list(&filters).await
        })
        .await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Unit, AppError> {
        let cache_key = keys::unit(&id);
        get_or_set(&*self.cache, &cache_key, keys::ttl::LOOKUP, || async {
            self.repo
                .find_by_id(id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Unit with ID {id} not found")))
        })
        .await
    }

    pub async fn create(
        &self,
        dto: CreateUnitDto,
        actor_id: Option<Uuid>,
    ) -> Result<Unit, AppError> {
        if self.repo.name_exists(&dto.name, None).await? {
            return Err(AppError::Conflict(format!(
                "Unit with name '{}' already exists",
                dto.name
            )));
        }
        if self
            .repo
            .abbreviation_exists(&dto.abbreviation, None)
            .await?
        {
            return Err(AppError::Conflict(format!(
                "Unit with abbreviation '{}' already exists",
                dto.abbreviation
            )));
        }
        let mut dto = dto;
        dto.r#type = Some(require_unit_type(dto.r#type)?);
        let unit = self.repo.create(&dto, actor_id).await?;
        self.invalidate_units(Some(unit.id)).await?;
        Ok(unit)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: UpdateUnitDto,
        actor_id: Option<Uuid>,
    ) -> Result<Unit, AppError> {
        if let Some(name) = &dto.name {
            if self.repo.name_exists(name, Some(id)).await? {
                return Err(AppError::Conflict(format!(
                    "Unit with name '{name}' already exists"
                )));
            }
        }
        if let Some(abbreviation) = &dto.abbreviation {
            if self
                .repo
                .abbreviation_exists(abbreviation, Some(id))
                .await?
            {
                return Err(AppError::Conflict(format!(
                    "Unit with abbreviation '{abbreviation}' already exists"
                )));
            }
        }
        let mut dto = dto;
        dto.r#type = optional_unit_type(dto.r#type)?;
        let unit = self.repo.update(id, &dto, actor_id).await?;
        self.invalidate_units(Some(id)).await?;
        Ok(unit)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        self.repo.soft_delete(id).await?;
        self.invalidate_units(Some(id)).await
    }
}

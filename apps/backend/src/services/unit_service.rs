use std::sync::Arc;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{CreateUnitDto, Unit, UnitFilterDto, UpdateUnitDto};
use crate::repositories::TenantUnitRepository;
use crate::tenancy::TenantDb;
use crate::validation::{optional_unit_type, require_unit_type};

#[derive(Clone, Default)]
pub struct UnitService;

impl UnitService {
    pub fn new() -> Self {
        Self
    }

    pub async fn list_tenant(
        &self,
        db: Arc<TenantDb>,
        filters: UnitFilterDto,
    ) -> Result<crate::dto::PaginationResult<Unit>, AppError> {
        TenantUnitRepository::new(db).list(&filters).await
    }

    pub async fn get_tenant(&self, db: Arc<TenantDb>, id: Uuid) -> Result<Unit, AppError> {
        TenantUnitRepository::new(db)
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Unit with ID {id} not found")))
    }

    pub async fn create_tenant(
        &self,
        db: Arc<TenantDb>,
        mut dto: CreateUnitDto,
        actor_id: Option<Uuid>,
    ) -> Result<Unit, AppError> {
        let repo = TenantUnitRepository::new(db);
        if repo.name_exists(&dto.name, None).await? {
            return Err(AppError::Conflict(format!(
                "Unit with name '{}' already exists",
                dto.name
            )));
        }
        if repo.abbreviation_exists(&dto.abbreviation, None).await? {
            return Err(AppError::Conflict(format!(
                "Unit with abbreviation '{}' already exists",
                dto.abbreviation
            )));
        }
        dto.r#type = Some(require_unit_type(dto.r#type)?);
        let unit = repo.create(&dto, actor_id).await?;
        Ok(unit)
    }

    pub async fn update_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
        mut dto: UpdateUnitDto,
        actor_id: Option<Uuid>,
    ) -> Result<Unit, AppError> {
        let repo = TenantUnitRepository::new(db);
        if let Some(name) = &dto.name {
            if repo.name_exists(name, Some(id)).await? {
                return Err(AppError::Conflict(format!(
                    "Unit with name '{name}' already exists"
                )));
            }
        }
        if let Some(abbreviation) = &dto.abbreviation {
            if repo.abbreviation_exists(abbreviation, Some(id)).await? {
                return Err(AppError::Conflict(format!(
                    "Unit with abbreviation '{abbreviation}' already exists"
                )));
            }
        }
        dto.r#type = optional_unit_type(dto.r#type)?;
        let unit = repo.update(id, &dto, actor_id).await?;
        Ok(unit)
    }

    pub async fn delete_tenant(&self, db: Arc<TenantDb>, id: Uuid) -> Result<(), AppError> {
        TenantUnitRepository::new(db).soft_delete(id).await?;
        Ok(())
    }
}

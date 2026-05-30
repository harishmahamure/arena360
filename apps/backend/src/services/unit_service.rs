use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{CreateUnitDto, Unit, UnitFilterDto, UpdateUnitDto};
use crate::repositories::UnitRepository;
use crate::validation::{optional_unit_type, require_unit_type};

pub struct UnitService {
    repo: UnitRepository,
}

impl UnitService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: UnitRepository::new(pool),
        }
    }

    pub async fn list(
        &self,
        filters: UnitFilterDto,
    ) -> Result<crate::dto::PaginationResult<Unit>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Unit, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Unit with ID {id} not found")))
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
        self.repo.create(&dto, actor_id).await
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
        self.repo.update(id, &dto, actor_id).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        self.repo.soft_delete(id).await
    }
}

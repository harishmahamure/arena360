use std::sync::Arc;
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{CreateGameDto, Game, GameFilterDto, UpdateGameDto};
use crate::repositories::TenantGameRepository;
use crate::tenancy::TenantDb;

#[derive(Clone, Default)]
pub struct GameService;

impl GameService {
    pub fn new() -> Self {
        Self
    }

    pub async fn list_tenant(
        &self,
        db: Arc<TenantDb>,
        filters: GameFilterDto,
    ) -> Result<PaginationResult<Game>, AppError> {
        TenantGameRepository::new(db).list(&filters).await
    }

    pub async fn get_tenant(&self, db: Arc<TenantDb>, id: Uuid) -> Result<Game, AppError> {
        TenantGameRepository::new(db)
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Game with ID {id} not found")))
    }

    pub async fn create_tenant(
        &self,
        db: Arc<TenantDb>,
        dto: CreateGameDto,
        actor_id: Option<Uuid>,
    ) -> Result<Game, AppError> {
        if dto.name.trim().is_empty() {
            return Err(AppError::BadRequest("Game name is required".into()));
        }
        let game = TenantGameRepository::new(db).create(&dto, actor_id).await?;
        Ok(game)
    }

    pub async fn update_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
        dto: UpdateGameDto,
        actor_id: Option<Uuid>,
    ) -> Result<Game, AppError> {
        let game = TenantGameRepository::new(db)
            .update(id, &dto, actor_id)
            .await?;
        Ok(game)
    }

    pub async fn delete_tenant(&self, db: Arc<TenantDb>, id: Uuid) -> Result<(), AppError> {
        TenantGameRepository::new(db).soft_delete(id).await?;
        Ok(())
    }
}

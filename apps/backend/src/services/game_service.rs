use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{CreateGameDto, Game, GameFilterDto, UpdateGameDto};
use crate::repositories::GameRepository;

#[derive(Clone)]
pub struct GameService {
    repo: GameRepository,
}

impl GameService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: GameRepository::new(pool),
        }
    }

    pub async fn list(&self, filters: GameFilterDto) -> Result<PaginationResult<Game>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Game, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Game with ID {id} not found")))
    }

    pub async fn create(&self, dto: CreateGameDto, actor_id: Option<Uuid>) -> Result<Game, AppError> {
        if dto.name.trim().is_empty() {
            return Err(AppError::BadRequest("Game name is required".to_string()));
        }
        self.repo.create(&dto, actor_id).await
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: UpdateGameDto,
        actor_id: Option<Uuid>,
    ) -> Result<Game, AppError> {
        self.repo.update(id, &dto, actor_id).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        self.repo.soft_delete(id).await
    }
}

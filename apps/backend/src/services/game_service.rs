use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{CreateGameDto, Game, GameFilterDto, UpdateGameDto};
use crate::repositories::GameRepository;

pub struct GameService {
    repo: GameRepository,
}

impl GameService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: GameRepository::new(pool),
        }
    }

    pub async fn list(
        &self,
        filters: GameFilterDto,
    ) -> Result<crate::dto::PaginationResult<Game>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Game, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Game with ID {id} not found")))
    }

    pub async fn create(&self, dto: CreateGameDto) -> Result<Game, AppError> {
        if self.repo.title_exists(&dto.title, None).await? {
            return Err(AppError::Conflict(format!(
                "Game with title '{}' already exists",
                dto.title
            )));
        }
        self.repo.create(&dto).await
    }

    pub async fn update(&self, id: Uuid, dto: UpdateGameDto) -> Result<Game, AppError> {
        if let Some(title) = &dto.title {
            if self.repo.title_exists(title, Some(id)).await? {
                return Err(AppError::Conflict(format!(
                    "Game with title '{title}' already exists"
                )));
            }
        }
        self.repo.update(id, &dto).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        self.get_by_id(id).await?;
        self.repo.soft_delete(id).await
    }
}

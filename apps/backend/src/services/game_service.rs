use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, get_or_set, keys, CacheService};
use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{CreateGameDto, Game, GameFilterDto, UpdateGameDto};
use crate::repositories::GameRepository;

#[derive(Clone)]
pub struct GameService {
    repo: GameRepository,
    cache: Arc<dyn CacheService>,
}

impl GameService {
    pub fn new(pool: PgPool, cache: Arc<dyn CacheService>) -> Self {
        Self {
            repo: GameRepository::new(pool),
            cache,
        }
    }

    async fn invalidate_games(&self, id: Option<Uuid>) -> Result<(), AppError> {
        let mut cache_keys = vec![keys::games_active().to_string()];
        if let Some(id) = id {
            cache_keys.push(keys::game(&id));
        }
        cache::invalidate(&*self.cache, &cache_keys).await?;
        self.cache.invalidate_prefix("games:list:").await
    }

    pub async fn list(&self, filters: GameFilterDto) -> Result<PaginationResult<Game>, AppError> {
        let cache_key = format!("games:list:{}", keys::filter_hash(&filters));
        get_or_set(&*self.cache, &cache_key, keys::ttl::LOOKUP, || async {
            self.repo.list(&filters).await
        })
        .await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Game, AppError> {
        let cache_key = keys::game(&id);
        get_or_set(&*self.cache, &cache_key, keys::ttl::LOOKUP, || async {
            self.repo
                .find_by_id(id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Game with ID {id} not found")))
        })
        .await
    }

    pub async fn create(
        &self,
        dto: CreateGameDto,
        actor_id: Option<Uuid>,
    ) -> Result<Game, AppError> {
        if dto.name.trim().is_empty() {
            return Err(AppError::BadRequest("Game name is required".to_string()));
        }
        let game = self.repo.create(&dto, actor_id).await?;
        self.invalidate_games(Some(game.id)).await?;
        Ok(game)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: UpdateGameDto,
        actor_id: Option<Uuid>,
    ) -> Result<Game, AppError> {
        let game = self.repo.update(id, &dto, actor_id).await?;
        self.invalidate_games(Some(id)).await?;
        Ok(game)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        self.repo.soft_delete(id).await?;
        self.invalidate_games(Some(id)).await
    }
}

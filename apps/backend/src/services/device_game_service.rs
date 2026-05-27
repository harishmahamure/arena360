use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    CreateDeviceGameDto, DeviceGameFilterDto, DeviceGameResponse, UpdateDeviceGameDto,
};
use crate::repositories::DeviceGameRepository;

pub struct DeviceGameService {
    repo: DeviceGameRepository,
}

impl DeviceGameService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: DeviceGameRepository::new(pool),
        }
    }

    pub async fn list(
        &self,
        filters: DeviceGameFilterDto,
    ) -> Result<crate::dto::PaginationResult<DeviceGameResponse>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn list_by_device(
        &self,
        device_id: Uuid,
        filters: DeviceGameFilterDto,
    ) -> Result<crate::dto::PaginationResult<DeviceGameResponse>, AppError> {
        if !self.repo.device_exists(device_id).await? {
            return Err(AppError::NotFound(format!(
                "Device with ID {device_id} not found"
            )));
        }
        self.repo.list_by_device(device_id, &filters).await
    }

    pub async fn list_by_game(
        &self,
        game_id: Uuid,
        filters: DeviceGameFilterDto,
    ) -> Result<crate::dto::PaginationResult<DeviceGameResponse>, AppError> {
        if !self.repo.game_exists(game_id).await? {
            return Err(AppError::NotFound(format!(
                "Game with ID {game_id} not found"
            )));
        }
        self.repo.list_by_game(game_id, &filters).await
    }

    pub async fn create(
        &self,
        dto: CreateDeviceGameDto,
        actor_id: Option<Uuid>,
    ) -> Result<DeviceGameResponse, AppError> {
        if !self.repo.device_exists(dto.device_id).await? {
            return Err(AppError::NotFound(format!(
                "Device with ID {} not found",
                dto.device_id
            )));
        }
        if !self.repo.game_exists(dto.game_id).await? {
            return Err(AppError::NotFound(format!(
                "Game with ID {} not found",
                dto.game_id
            )));
        }

        let created = self.repo.create(&dto, actor_id).await?;
        self.repo
            .find_by_id(created.id)
            .await?
            .ok_or_else(|| AppError::Internal("Failed to load created device-game".to_string()))
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<DeviceGameResponse, AppError> {
        self.repo.find_by_id(id).await?.ok_or_else(|| {
            AppError::NotFound(format!("Device-game assignment with ID {id} not found"))
        })
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: UpdateDeviceGameDto,
        actor_id: Option<Uuid>,
    ) -> Result<DeviceGameResponse, AppError> {
        self.repo.update(id, &dto, actor_id).await?;
        self.get_by_id(id).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        self.repo.soft_delete(id).await
    }
}

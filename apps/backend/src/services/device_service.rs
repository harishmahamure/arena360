use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    CreateDeviceDto, Device, DeviceFilterDto, UpdateDeviceDto, UpdateDeviceStatusDto,
};
use crate::repositories::DeviceRepository;
use crate::services::EventService;

#[derive(Clone)]
pub struct DeviceService {
    repo: DeviceRepository,
    events: EventService,
}

impl DeviceService {
    pub fn new(pool: PgPool, events: EventService) -> Self {
        Self {
            repo: DeviceRepository::new(pool),
            events,
        }
    }

    pub async fn list(
        &self,
        filters: DeviceFilterDto,
    ) -> Result<crate::dto::PaginationResult<Device>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Device, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Device with ID {id} not found")))
    }

    pub async fn create(
        &self,
        dto: CreateDeviceDto,
        actor_id: Option<Uuid>,
    ) -> Result<Device, AppError> {
        if self.repo.name_exists(&dto.name, None).await? {
            return Err(AppError::Conflict(format!(
                "Device with name '{}' already exists",
                dto.name
            )));
        }
        let device = self.repo.create(&dto, actor_id).await?;
        self.events
            .publish_device_status(&device.id.to_string(), &device.status);
        Ok(device)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: UpdateDeviceDto,
        actor_id: Option<Uuid>,
    ) -> Result<Device, AppError> {
        if let Some(name) = &dto.name {
            if self.repo.name_exists(name, Some(id)).await? {
                return Err(AppError::Conflict(format!(
                    "Device with name '{name}' already exists"
                )));
            }
        }
        let device = self.repo.update(id, &dto, actor_id).await?;
        self.events
            .publish_device_status(&device.id.to_string(), &device.status);
        Ok(device)
    }

    pub async fn update_status(
        &self,
        id: Uuid,
        dto: UpdateDeviceStatusDto,
    ) -> Result<Device, AppError> {
        let device = self.repo.update_status(id, &dto.status).await?;
        self.events
            .publish_device_status(&device.id.to_string(), &device.status);
        Ok(device)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        self.repo.soft_delete(id).await
    }
}

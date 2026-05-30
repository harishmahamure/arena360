use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::{DeviceRegistrationCodeResponseDto, RegisterDeviceDto};
use crate::error::AppError;
use crate::models::{
    CreateDeviceDto, Device, DeviceFilterDto, UpdateDeviceDto, UpdateDeviceStatusDto,
};
use crate::realtime::OutboxService;
use crate::repositories::DeviceRepository;
use crate::services::EventService;
use crate::validation::{
    optional_device_status, optional_device_sub_type, optional_device_type, require_device_sub_type,
    require_device_type,
};

#[derive(Clone)]
pub struct DeviceService {
    repo: DeviceRepository,
    events: EventService,
    outbox: OutboxService,
}

impl DeviceService {
    pub fn new(pool: PgPool, events: EventService, outbox: OutboxService) -> Self {
        Self {
            repo: DeviceRepository::new(pool),
            events,
            outbox,
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
        let dto = prepare_create_dto(dto)?;
        if self.repo.name_exists(&dto.name, None).await? {
            return Err(AppError::Conflict(format!(
                "Device with name '{}' already exists",
                dto.name
            )));
        }
        let device = self.repo.create(&dto, actor_id).await?;
        let device = self
            .issue_registration_code(device.id, actor_id)
            .await?;
        self.events
            .publish_device_status(&device.id.to_string(), &device.status);
        self.publish_device_ws(&device).await;
        Ok(device)
    }

    pub async fn issue_registration_code(
        &self,
        id: Uuid,
        actor_id: Option<Uuid>,
    ) -> Result<Device, AppError> {
        self.get_by_id(id).await?;
        let code = new_registration_code();
        let expires_at = Utc::now() + Duration::hours(24);
        self.repo
            .set_registration_code(id, &code, expires_at, actor_id)
            .await
    }

    pub async fn registration_code_response(
        &self,
        id: Uuid,
        actor_id: Option<Uuid>,
    ) -> Result<DeviceRegistrationCodeResponseDto, AppError> {
        let device = self.issue_registration_code(id, actor_id).await?;
        let expires_at = device
            .registration_code_expires_at
            .ok_or_else(|| AppError::Internal("Registration code expiry missing".to_string()))?;
        let code = device
            .registration_code
            .ok_or_else(|| AppError::Internal("Registration code missing".to_string()))?;
        Ok(DeviceRegistrationCodeResponseDto {
            registrationCode: code,
            expiresAt: expires_at.to_rfc3339(),
        })
    }

    pub async fn register_kiosk(&self, mut dto: RegisterDeviceDto) -> Result<Device, AppError> {
        dto.deviceType = optional_device_type(dto.deviceType)?;
        dto.deviceSubType = optional_device_sub_type(dto.deviceSubType)?;

        let pending = self
            .repo
            .find_pending_by_registration_code(&dto.registrationCode)
            .await?
            .ok_or_else(|| AppError::unauthorized_code("DEVICE_REGISTRATION_INVALID"))?;

        let fingerprint_json = serde_json::to_string(&dto.fingerprint)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        self.repo
            .redeem_registration(pending.id, &dto, &fingerprint_json)
            .await
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
        let dto = prepare_update_dto(dto)?;
        let device = self.repo.update(id, &dto, actor_id).await?;
        self.events
            .publish_device_status(&device.id.to_string(), &device.status);
        self.publish_device_ws(&device).await;
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
        self.publish_device_ws(&device).await;
        Ok(device)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        self.repo.soft_delete(id).await
    }

    async fn publish_device_ws(&self, device: &Device) {
        let channel = format!("device:{}", device.id);
        let payload = serde_json::json!({
            "deviceId": device.id.to_string(),
            "status": device.status
        });
        let _ = self.outbox.publish("staff", "device.status_changed", payload.clone(), None, None, false).await;
        let _ = self.outbox.publish(&channel, "device.status_changed", payload, None, None, false).await;
    }
}

fn new_registration_code() -> String {
    let raw = Uuid::new_v4().to_string().replace('-', "");
    let upper = raw.to_uppercase();
    format!("{}-{}", &upper[..3], &upper[3..6])
}

fn prepare_create_dto(mut dto: CreateDeviceDto) -> Result<CreateDeviceDto, AppError> {
    dto.device_type = Some(require_device_type(dto.device_type)?);
    dto.device_sub_type = Some(require_device_sub_type(dto.device_sub_type)?);
    Ok(dto)
}

fn prepare_update_dto(mut dto: UpdateDeviceDto) -> Result<UpdateDeviceDto, AppError> {
    dto.device_type = optional_device_type(dto.device_type)?;
    dto.device_sub_type = optional_device_sub_type(dto.device_sub_type)?;
    dto.status = optional_device_status(dto.status)?;
    Ok(dto)
}

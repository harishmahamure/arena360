use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::{DeviceFingerprintDto, ProvisionDeviceDto};
use crate::error::AppError;
use crate::models::{
    CreateDeviceDto, Device, DeviceFilterDto, UpdateDeviceDto, UpdateDeviceStatusDto,
};
use crate::realtime::OutboxService;
use crate::repositories::DeviceRepository;
use crate::services::EventService;
use crate::validation::{
    optional_device_status, optional_device_sub_type, optional_device_type,
    require_device_sub_type, require_device_type,
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
        self.events
            .publish_device_status(&device.id.to_string(), &device.status);
        self.publish_device_ws(&device).await;
        Ok(device)
    }

    /// Admin-authorized provisioning (DRAFT-0023): the admin is already
    /// authenticated; create a registered device with its fingerprint snapshot.
    pub async fn provision(
        &self,
        mut dto: ProvisionDeviceDto,
        actor_id: Option<Uuid>,
    ) -> Result<Device, AppError> {
        if dto.name.trim().is_empty() {
            return Err(AppError::BadRequest("Device name is required".to_string()));
        }
        dto.deviceType = Some(require_device_type(dto.deviceType)?);
        dto.deviceSubType = Some(require_device_sub_type(dto.deviceSubType)?);

        if self.repo.name_exists(&dto.name, None).await? {
            return Err(AppError::Conflict(format!(
                "Device with name '{}' already exists",
                dto.name
            )));
        }

        let fingerprint_json = serde_json::to_string(&dto.fingerprint)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let device = self.repo.provision(&dto, &fingerprint_json, actor_id).await?;
        self.events
            .publish_device_status(&device.id.to_string(), &device.status);
        self.publish_device_ws(&device).await;
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

    /// Enforce the fingerprint drift policy (ADR-0017, US-KREG-002/003):
    /// tolerate a single changed component (persist the refreshed snapshot),
    /// reject when two or more of MAC / serial / BIOS UUID differ.
    pub async fn verify_fingerprint_drift(
        &self,
        device: &Device,
        presented: &DeviceFingerprintDto,
    ) -> Result<(), AppError> {
        let Some(stored_json) = device.registered_kiosk.as_deref() else {
            // No stored fingerprint yet (legacy/registered without one): accept and store.
            if let Ok(json) = serde_json::to_string(presented) {
                let _ = self.repo.update_fingerprint(device.id, &json).await;
            }
            return Ok(());
        };

        let stored: DeviceFingerprintDto = match serde_json::from_str(stored_json) {
            Ok(value) => value,
            Err(_) => return Ok(()), // unparseable legacy snapshot — don't lock the player out
        };

        match fingerprint_drift_count(&stored, presented) {
            0 => Ok(()),
            1 => {
                if let Ok(json) = serde_json::to_string(presented) {
                    let _ = self.repo.update_fingerprint(device.id, &json).await;
                }
                tracing::warn!(device_id = %device.id, "Single-component fingerprint drift; snapshot refreshed");
                Ok(())
            }
            _ => Err(AppError::forbidden_code("DEVICE_FINGERPRINT_MISMATCH")),
        }
    }

    async fn publish_device_ws(&self, device: &Device) {
        let channel = format!("device:{}", device.id);
        let payload = serde_json::json!({
            "deviceId": device.id.to_string(),
            "status": device.status
        });
        let _ = self
            .outbox
            .publish(
                "staff",
                "device.status_changed",
                payload.clone(),
                None,
                None,
                false,
            )
            .await;
        let _ = self
            .outbox
            .publish(
                &channel,
                "device.status_changed",
                payload,
                None,
                None,
                false,
            )
            .await;
    }
}

/// Count how many of the three identifying fingerprint components differ.
/// Comparison is case-insensitive and ignores `platform` / `collectedAt`.
pub fn fingerprint_drift_count(
    stored: &DeviceFingerprintDto,
    presented: &DeviceFingerprintDto,
) -> usize {
    let diff = |a: &str, b: &str| !a.trim().eq_ignore_ascii_case(b.trim());
    let mut count = 0;
    if diff(&stored.mac, &presented.mac) {
        count += 1;
    }
    if diff(&stored.serial, &presented.serial) {
        count += 1;
    }
    if diff(&stored.biosUuid, &presented.biosUuid) {
        count += 1;
    }
    count
}

#[cfg(test)]
mod fingerprint_tests {
    use super::*;

    fn fp(mac: &str, serial: &str, bios: &str) -> DeviceFingerprintDto {
        DeviceFingerprintDto {
            mac: mac.to_string(),
            serial: serial.to_string(),
            biosUuid: bios.to_string(),
            platform: "windows".to_string(),
            collectedAt: "2026-05-30T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn identical_fingerprints_have_no_drift() {
        let a = fp("AA:BB", "SN1", "UUID1");
        assert_eq!(fingerprint_drift_count(&a, &a), 0);
    }

    #[test]
    fn case_insensitive_match() {
        let a = fp("aa:bb", "sn1", "uuid1");
        let b = fp("AA:BB", "SN1", "UUID1");
        assert_eq!(fingerprint_drift_count(&a, &b), 0);
    }

    #[test]
    fn single_component_change_counts_one() {
        let a = fp("AA:BB", "SN1", "UUID1");
        let b = fp("AA:BB", "SN2", "UUID1");
        assert_eq!(fingerprint_drift_count(&a, &b), 1);
    }

    #[test]
    fn two_component_change_counts_two() {
        let a = fp("AA:BB", "SN1", "UUID1");
        let b = fp("CC:DD", "SN2", "UUID1");
        assert_eq!(fingerprint_drift_count(&a, &b), 2);
    }
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

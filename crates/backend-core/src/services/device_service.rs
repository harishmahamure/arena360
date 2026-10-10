use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, keys, CacheService};
use crate::dto::{DeviceFingerprintDto, ProvisionDeviceDto};
use crate::error::AppError;
use crate::models::{
    CreateDeviceDto, Device, DeviceFilterDto, UpdateDeviceDto, UpdateDeviceStatusDto,
};
use crate::repositories::TenantDeviceRepository;
use crate::services::EventService;
use crate::tenancy::TenantDb;
use crate::validation::{
    optional_device_status, optional_device_sub_type, optional_device_type,
    require_device_sub_type, require_device_type,
};

#[derive(Clone)]
pub struct DeviceService {
    events: EventService,
    cache: Arc<dyn CacheService>,
}

impl DeviceService {
    pub fn new(events: EventService, cache: Arc<dyn CacheService>) -> Self {
        Self { events, cache }
    }

    async fn invalidate_device_cache(&self, device_id: &Uuid) {
        let _ = cache::invalidate(&*self.cache, &[keys::session_device(device_id)]).await;
        let _ = self
            .cache
            .invalidate_prefix(keys::SESSIONS_LIST_PREFIX)
            .await;
        let _ = cache::invalidate_stats(&*self.cache).await;
    }

    pub async fn list_tenant(
        &self,
        db: Arc<TenantDb>,
        filters: DeviceFilterDto,
        allowed_locations: Vec<Uuid>,
    ) -> Result<crate::dto::PaginationResult<Device>, AppError> {
        TenantDeviceRepository::new(db)
            .list(&filters, &allowed_locations)
            .await
    }

    pub async fn get_tenant(&self, db: Arc<TenantDb>, id: Uuid) -> Result<Device, AppError> {
        TenantDeviceRepository::new(db)
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Device with ID {id} not found")))
    }

    pub async fn create_tenant(
        &self,
        db: Arc<TenantDb>,
        dto: CreateDeviceDto,
        actor_id: Option<Uuid>,
    ) -> Result<Device, AppError> {
        let dto = prepare_create_dto(dto)?;
        let repo = TenantDeviceRepository::new(db);
        if repo.name_exists(&dto.name, None).await? {
            return Err(AppError::Conflict(format!(
                "Device with name '{}' already exists",
                dto.name
            )));
        }
        let device = repo.create(&dto, actor_id).await?;
        self.after_tenant_mutation(&device).await;
        Ok(device)
    }

    pub async fn update_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
        dto: UpdateDeviceDto,
        actor_id: Option<Uuid>,
    ) -> Result<Device, AppError> {
        let repo = TenantDeviceRepository::new(db);
        if let Some(name) = dto.name.as_deref() {
            if repo.name_exists(name, Some(id)).await? {
                return Err(AppError::Conflict("Device name already exists".into()));
            }
        }
        let device = repo.update(id, &prepare_update_dto(dto)?, actor_id).await?;
        self.after_tenant_mutation(&device).await;
        Ok(device)
    }

    pub async fn update_status_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
        dto: UpdateDeviceStatusDto,
    ) -> Result<Device, AppError> {
        let status = optional_device_status(Some(dto.status))?
            .ok_or_else(|| AppError::BadRequest("Device status is required".into()))?;
        let device = TenantDeviceRepository::new(db)
            .update_status(id, &status)
            .await?;
        self.after_tenant_mutation(&device).await;
        Ok(device)
    }

    pub async fn delete_tenant(&self, db: Arc<TenantDb>, id: Uuid) -> Result<(), AppError> {
        // The repository performs the active-session check inside the fenced
        // immediate transaction so a session cannot race this deletion.
        TenantDeviceRepository::new(db).soft_delete(id).await?;
        self.invalidate_device_cache(&id).await;
        Ok(())
    }

    pub async fn provision_tenant(
        &self,
        db: Arc<TenantDb>,
        mut dto: ProvisionDeviceDto,
        actor_id: Option<Uuid>,
    ) -> Result<Device, AppError> {
        let location_id = dto
            .locationId
            .ok_or_else(|| AppError::bad_request_code("LOCATION_REQUIRED", None))?;
        if dto.name.trim().is_empty() {
            return Err(AppError::BadRequest("Device name is required".into()));
        }
        dto.deviceType = Some(require_device_type(dto.deviceType)?);
        dto.deviceSubType = Some(require_device_sub_type(dto.deviceSubType)?);
        dto.serialNumber = Some(dto.fingerprint.mac.trim().to_string());
        let fingerprint = serde_json::to_string(&dto.fingerprint)
            .map_err(|error| AppError::BadRequest(error.to_string()))?;
        let repo = TenantDeviceRepository::new(db);
        let existing = if is_unusable_mac(&dto.fingerprint.mac) {
            None
        } else {
            repo.find_registered_by_mac(&dto.fingerprint.mac).await?
        };
        if let Some(existing) = &existing {
            let requested = location_id;
            if requested != existing.location_id {
                return Err(AppError::Conflict(
                    "Move the device in admin before provisioning it at another location".into(),
                ));
            }
            if !self.fingerprint_compatible(existing, &dto.fingerprint)? {
                return Err(AppError::Conflict(
                    "This hardware fingerprint does not match the registered device".into(),
                ));
            }
        } else if repo.name_exists(&dto.name, None).await? {
            return Err(AppError::Conflict(format!(
                "Device with name '{}' already exists",
                dto.name
            )));
        }
        let device = repo
            .provision(
                existing.map(|value| value.id),
                dto.name,
                dto.serialNumber,
                dto.deviceType.unwrap_or_else(|| "OTHER".into()),
                dto.deviceSubType.unwrap_or_else(|| "OTHER".into()),
                dto.location,
                location_id,
                fingerprint,
                actor_id,
            )
            .await?;
        self.after_tenant_mutation(&device).await;
        Ok(device)
    }

    pub async fn verify_fingerprint_drift_tenant(
        &self,
        db: Arc<TenantDb>,
        device: &Device,
        presented: &DeviceFingerprintDto,
    ) -> Result<(), AppError> {
        let Some(stored) = device.registered_kiosk.as_deref() else {
            let json = serde_json::to_string(presented)
                .map_err(|error| AppError::BadRequest(error.to_string()))?;
            return TenantDeviceRepository::new(db)
                .update_fingerprint(device.id, json)
                .await;
        };
        let Ok(stored) = serde_json::from_str::<DeviceFingerprintDto>(stored) else {
            return Ok(());
        };
        match fingerprint_drift_count(&stored, presented) {
            0 => Ok(()),
            1 => {
                let json = serde_json::to_string(presented)
                    .map_err(|error| AppError::BadRequest(error.to_string()))?;
                TenantDeviceRepository::new(db)
                    .update_fingerprint(device.id, json)
                    .await
            }
            _ => Err(AppError::forbidden_code("DEVICE_FINGERPRINT_MISMATCH")),
        }
    }

    pub(crate) async fn after_tenant_mutation(&self, device: &Device) {
        self.events
            .publish_device_status(&device.id.to_string(), &device.status);
        self.invalidate_device_cache(&device.id).await;
    }

    fn fingerprint_compatible(
        &self,
        existing: &Device,
        presented: &DeviceFingerprintDto,
    ) -> Result<bool, AppError> {
        let Some(stored_json) = existing.registered_kiosk.as_deref() else {
            return Ok(true);
        };
        let stored: DeviceFingerprintDto = match serde_json::from_str(stored_json) {
            Ok(value) => value,
            Err(_) => return Ok(true),
        };
        Ok(mac_addresses_match(&stored, presented))
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

/// Case-insensitive MAC equality for provisioning / reprovision.
pub fn mac_addresses_match(
    stored: &DeviceFingerprintDto,
    presented: &DeviceFingerprintDto,
) -> bool {
    stored.mac.trim().eq_ignore_ascii_case(presented.mac.trim())
}

/// MAC values that must not be used as a station lookup key.
pub fn is_unusable_mac(mac: &str) -> bool {
    let normalized = mac.trim().to_ascii_lowercase();
    normalized.is_empty()
        || normalized == "00:00:00:00:00:00"
        || normalized == "00-00-00-00-00-00"
        || normalized == "aa:bb:cc:dd:ee:ff"
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

    #[test]
    fn fingerprint_compatible_allows_single_drift() {
        let stored = fp("AA:BB", "SN1", "UUID1");
        let presented = fp("AA:BB", "SN2", "UUID1");
        assert_eq!(fingerprint_drift_count(&stored, &presented), 1);
    }

    #[test]
    fn same_serial_different_mac_is_not_reprovision() {
        let pc_a = fp("AA:11", "OEM-SERIAL", "BIOS-A");
        let pc_b = fp("BB:22", "OEM-SERIAL", "BIOS-A");
        assert!(!mac_addresses_match(&pc_a, &pc_b));
    }

    #[test]
    fn same_mac_is_reprovision_compatible() {
        let stored = fp("AA:BB:CC:DD:EE:01", "OEM-SERIAL", "BIOS-1");
        let presented = fp("aa:bb:cc:dd:ee:01", "To be filled by O.E.M.", "BIOS-2");
        assert!(mac_addresses_match(&stored, &presented));
    }

    #[test]
    fn unusable_mac_detected() {
        assert!(is_unusable_mac("00:00:00:00:00:00"));
        assert!(is_unusable_mac("AA:BB:CC:DD:EE:FF"));
        assert!(!is_unusable_mac("10:20:30:40:50:60"));
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

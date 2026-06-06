use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::Device;

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DeviceFingerprintDto {
    pub mac: String,
    pub serial: String,
    pub biosUuid: String,
    pub platform: String,
    pub collectedAt: String,
}

/// Admin-authorized device provisioning payload (DRAFT-0023). Sent with an admin
/// bearer token to `POST /devices/provision`.
#[allow(non_snake_case)]
#[derive(Debug, Deserialize, ToSchema)]
pub struct ProvisionDeviceDto {
    pub fingerprint: DeviceFingerprintDto,
    pub name: String,
    pub serialNumber: Option<String>,
    pub deviceType: Option<String>,
    pub deviceSubType: Option<String>,
    pub location: Option<String>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, ToSchema)]
pub struct RegisteredDeviceDto {
    pub id: String,
    pub name: String,
    pub registrationStatus: String,
    pub deviceType: String,
    pub deviceSubType: String,
    pub status: String,
}

impl From<Device> for RegisteredDeviceDto {
    fn from(device: Device) -> Self {
        Self {
            id: device.id.to_string(),
            name: device.name,
            registrationStatus: device.registration_status,
            deviceType: device.device_type,
            deviceSubType: device.device_sub_type,
            status: device.status,
        }
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, ToSchema)]
pub struct DeviceRegisterResponseDto {
    pub accessToken: String,
    pub device: RegisteredDeviceDto,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, ToSchema)]
pub struct StartKioskSessionDto {
    /// Optional balance to spend; when omitted the backend picks the best
    /// usable balance for the device type.
    pub balanceId: Option<String>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, ToSchema)]
pub struct KioskSessionResponseDto {
    pub sessionId: String,
    pub balanceId: String,
    pub deviceId: String,
    pub startTime: String,
    pub remainingMinutes: f64,
    /// True when an existing open session on this device was resumed (crash recovery).
    pub resumed: bool,
    /// Set when the session has been closed (auto, voluntary, force, offline_reconcile).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endTime: Option<String>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct EndKioskSessionDto {
    /// One of: voluntary, auto, force, offline_reconcile (defaults to voluntary).
    pub reason: Option<String>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, ToSchema)]
pub struct PlayerLoginDto {
    pub username: String,
    pub password: String,
    /// Current hardware fingerprint; when present the backend enforces the
    /// drift policy (ADR-0017) before issuing a player token.
    pub fingerprint: Option<DeviceFingerprintDto>,
}

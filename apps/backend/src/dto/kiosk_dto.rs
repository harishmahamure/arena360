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

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterDeviceDto {
    pub registrationCode: String,
    pub fingerprint: DeviceFingerprintDto,
    pub name: String,
    pub serialNumber: Option<String>,
    pub deviceType: Option<String>,
    pub deviceSubType: Option<String>,
    pub location: Option<String>,
    pub ipAddress: Option<String>,
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
#[derive(Debug, Serialize, ToSchema)]
pub struct DeviceRegistrationCodeResponseDto {
    pub registrationCode: String,
    pub expiresAt: String,
}

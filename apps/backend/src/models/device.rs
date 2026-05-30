use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: Uuid,
    pub name: String,
    pub serial_number: Option<String>,
    pub local_ip_address: Option<String>,
    pub device_type: String,
    pub device_sub_type: String,
    pub location: Option<String>,
    pub status: String,
    pub registered_kiosk: Option<String>,
    pub registration_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_code_expires_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateDeviceDto {
    pub name: String,
    pub serial_number: Option<String>,
    pub local_ip_address: Option<String>,
    pub device_type: Option<String>,
    pub device_sub_type: Option<String>,
    pub location: Option<String>,
    pub status: Option<String>,
    pub registration_status: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDeviceDto {
    pub name: Option<String>,
    pub serial_number: Option<String>,
    pub local_ip_address: Option<String>,
    pub device_type: Option<String>,
    pub device_sub_type: Option<String>,
    pub location: Option<String>,
    pub status: Option<String>,
    pub registration_status: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateDeviceStatusDto {
    pub status: String,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFilterDto {
    pub status: Option<String>,
    pub device_type: Option<String>,
    pub device_sub_type: Option<String>,
    pub location: Option<String>,
    pub name: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

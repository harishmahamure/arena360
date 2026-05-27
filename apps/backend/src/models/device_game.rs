use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceGame {
    pub id: Uuid,
    pub device_id: Uuid,
    pub game_id: Uuid,
    pub installation_date: DateTime<Utc>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceGameDevice {
    pub id: Uuid,
    pub name: String,
    pub device_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceGameGame {
    pub id: Uuid,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceGameResponse {
    pub id: Uuid,
    pub device_id: Uuid,
    pub game_id: Uuid,
    pub installation_date: DateTime<Utc>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<DeviceGameDevice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game: Option<DeviceGameGame>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateDeviceGameDto {
    pub device_id: Uuid,
    pub game_id: Uuid,
    pub installation_date: Option<DateTime<Utc>>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, Default, Clone, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct DeviceGameFilterDto {
    pub device_id: Option<Uuid>,
    pub game_id: Option<Uuid>,
    pub is_active: Option<i64>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

#[derive(Debug, FromRow)]
pub struct DeviceGameRow {
    pub id: Uuid,
    pub device_id: Uuid,
    pub game_id: Uuid,
    pub installation_date: DateTime<Utc>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub device_name: Option<String>,
    pub device_type: Option<String>,
    pub device_location: Option<String>,
    pub game_title: Option<String>,
    pub game_genre: Option<String>,
}

impl DeviceGameRow {
    pub fn into_response(self) -> DeviceGameResponse {
        let device = self.device_name.map(|name| DeviceGameDevice {
            id: self.device_id,
            name,
            device_type: self.device_type.unwrap_or_else(|| "other".to_string()),
            location: self.device_location,
        });
        let game = self.game_title.map(|title| DeviceGameGame {
            id: self.game_id,
            title,
            genre: self.game_genre,
        });

        DeviceGameResponse {
            id: self.id,
            device_id: self.device_id,
            game_id: self.game_id,
            installation_date: self.installation_date,
            is_active: self.is_active,
            created_at: self.created_at,
            updated_at: self.updated_at,
            device,
            game,
        }
    }
}

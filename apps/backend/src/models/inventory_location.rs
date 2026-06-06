use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InventoryLocation {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub is_active: bool,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateInventoryLocationDto {
    pub name: String,
    pub kind: String,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInventoryLocationDto {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct InventoryLocationFilterDto {
    pub kind: Option<String>,
    pub is_active: Option<bool>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

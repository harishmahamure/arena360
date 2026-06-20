use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
    pub id: Uuid,
    pub key: String,
    pub value: serde_json::Value,
    pub category: String,
    pub description: Option<String>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpsertConfigDto {
    pub value: serde_json::Value,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFilterDto {
    pub category: Option<String>,
    pub key: Option<String>,
}

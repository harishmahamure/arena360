use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

pub const DEFAULT_ORGANIZATION_ID: Uuid = Uuid::from_u128(0x00000000_0000_4000_8000_000000000001);
pub const DEFAULT_VENUE_LOCATION_ID: Uuid = Uuid::from_u128(0x00000000_0000_4000_8000_000000000002);

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SettingScope {
    Organization,
    Location,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SettingValueType {
    String,
    Number,
    Integer,
    Boolean,
    Uuid,
    Timezone,
    Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SettingDefinition {
    pub key: String,
    pub category: String,
    pub description: String,
    pub value_type: SettingValueType,
    pub default_value: serde_json::Value,
    pub allowed_scopes: Vec<SettingScope>,
    pub validation: serde_json::Value,
    pub sensitive: bool,
    pub owner: String,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SettingOverride {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub location_id: Option<Uuid>,
    pub key: String,
    pub value: serde_json::Value,
    pub revision: i64,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedSetting {
    pub key: String,
    pub value: serde_json::Value,
    pub source_scope: String,
    pub source_id: Option<Uuid>,
    pub revision: i64,
    pub updated_at: Option<DateTime<Utc>>,
    pub overridden: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpsertSettingOverrideDto {
    pub location_id: Option<Uuid>,
    pub value: serde_json::Value,
    pub reason: String,
    pub expected_revision: Option<i64>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveSettingsQuery {
    pub location_id: Option<Uuid>,
    pub category: Option<String>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct SettingHistoryQuery {
    pub location_id: Option<Uuid>,
    pub key: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSettingOverrideQuery {
    pub location_id: Option<Uuid>,
    pub expected_revision: Option<i64>,
    pub reason: String,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationSnapshotQuery {
    pub location_id: Option<Uuid>,
    pub since_revision: Option<i64>,
}

#[derive(Debug, Clone, FromRow, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SettingRevision {
    pub id: i64,
    pub organization_id: Uuid,
    pub location_id: Option<Uuid>,
    pub key: String,
    pub revision: i64,
    pub operation: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub reason: String,
    pub actor_user_id: Option<Uuid>,
    pub request_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationSnapshot {
    pub organization_id: Uuid,
    pub location_id: Option<Uuid>,
    pub revision: i64,
    pub etag: String,
    pub generated_at: DateTime<Utc>,
    pub settings: Vec<ResolvedSetting>,
}

#[derive(Debug, Clone, FromRow, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationMembershipContext {
    pub organization_id: Uuid,
    pub role: String,
    pub permissions: serde_json::Value,
}

#[derive(Debug, Clone, FromRow, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VenueLocation {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub slug: String,
    pub name: String,
    pub timezone: String,
    pub currency: String,
    pub is_active: bool,
}

use chrono::{DateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PricingRuleSet {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub location_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub active_version_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PricingRuleVersion {
    pub id: Uuid,
    pub rule_set_id: Uuid,
    pub version: i32,
    pub status: String,
    pub policy: serde_json::Value,
    pub simulation_hash: Option<String>,
    pub validated_at: Option<DateTime<Utc>>,
    pub effective_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
    pub published_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PricingRuleSetDraft {
    pub rule_set: PricingRuleSet,
    pub version: PricingRuleVersion,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PricingPolicy {
    /// Decimal amount encoded as a string to avoid floating-point money errors.
    pub base_rate: String,
    #[serde(default)]
    pub rules: Vec<PricingRule>,
    #[serde(default = "default_rounding_scale")]
    pub rounding_scale: u32,
    pub minimum_price: Option<String>,
    pub maximum_price: Option<String>,
}

fn default_rounding_scale() -> u32 {
    2
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PricingRule {
    pub id: String,
    pub name: String,
    pub priority: i32,
    #[serde(default)]
    pub device_types: Vec<String>,
    #[serde(default)]
    pub weekdays: Vec<u8>,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub action: PricingAction,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PricingAction {
    Fixed { value: String },
    Multiplier { value: String },
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatePricingRuleSetDto {
    pub location_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub policy: PricingPolicy,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatePricingRuleVersionDto {
    pub policy: PricingPolicy,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishPricingRuleVersionDto {
    pub effective_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PricingSimulationDto {
    pub location_id: Option<Uuid>,
    pub device_type: Option<String>,
    pub at: DateTime<Utc>,
    pub base_rate: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PricingTraceStep {
    pub rule_id: String,
    pub rule_name: String,
    pub action: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PricingSimulationResult {
    pub base_rate: String,
    pub final_price: String,
    pub currency: String,
    pub timezone: String,
    pub trace: Vec<PricingTraceStep>,
    pub simulation_hash: String,
}

#[derive(Debug, Deserialize, Default, IntoParams, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PricingRuleSetQuery {
    pub location_id: Option<Uuid>,
}

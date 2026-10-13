use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum OnboardingStep {
    Venue,
    Settings,
    Stations,
    Review,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingProgress {
    pub current_step: OnboardingStep,
    pub revision: i64,
    pub started_at: Option<String>,
    pub settings_reviewed_at: Option<String>,
    pub completed_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveOnboardingDto {
    pub expected_revision: i64,
    pub current_step: OnboardingStep,
    #[serde(default)]
    pub review_settings: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompleteOnboardingDto {
    pub expected_revision: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingCheck {
    pub key: String,
    pub label: String,
    pub ready: bool,
    pub action_path: String,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingLocation {
    pub id: String,
    pub name: String,
    pub timezone: String,
    pub currency: String,
    pub station_count: i64,
    pub compatible_plan_count: i64,
    pub operator_ready: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingReadiness {
    pub ready: bool,
    pub checks: Vec<OnboardingCheck>,
    pub locations: Vec<OnboardingLocation>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingSnapshot {
    pub progress: OnboardingProgress,
    pub readiness: OnboardingReadiness,
}

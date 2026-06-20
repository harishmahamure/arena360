use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: Uuid,
    #[serde(skip_serializing)]
    pub email: Option<String>,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    pub is_active: bool,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone_number: Option<String>,
    pub role: Option<String>,
    #[serde(default)]
    #[sqlx(default)]
    pub credit_limit: f64,
    pub session_otp_id: Option<String>,
    #[serde(skip_serializing)]
    pub session_otp: Option<String>,
    #[serde(skip_serializing)]
    pub totp_secret: Option<String>,
    pub totp_enabled: bool,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl User {
    pub fn to_auth_user(&self) -> crate::dto::AuthUserDto {
        crate::dto::AuthUserDto {
            id: self.id.to_string(),
            username: self.username.clone(),
            phoneNumber: self.phone_number.clone(),
            firstName: self.first_name.clone(),
            lastName: self.last_name.clone(),
            role: self.role.clone().unwrap_or_else(|| "player".to_string()),
            isActive: self.is_active,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerifyTotpSetupDto {
    pub code: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TotpSetupResponseDto {
    pub secret: String,
    pub otpauthUri: String,
    pub totpEnabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserDto {
    pub username: Option<String>,
    pub phone_number: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub role: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct UserFilterDto {
    pub username: Option<String>,
    pub phone_number: Option<String>,
    pub is_active: Option<i64>,
    pub role: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

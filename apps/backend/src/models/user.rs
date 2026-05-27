use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: Uuid,
    pub email: Option<String>,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    pub is_active: bool,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone_number: Option<String>,
    pub role: Option<String>,
    pub session_otp_id: Option<String>,
    #[serde(skip_serializing)]
    pub session_otp: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl User {
    pub fn to_auth_user(&self) -> crate::dto::AuthUserDto {
        crate::dto::AuthUserDto {
            id: self.id.to_string(),
            email: self.email.clone(),
            username: self.username.clone(),
            firstName: self.first_name.clone(),
            lastName: self.last_name.clone(),
            role: self.role.clone().unwrap_or_else(|| "player".to_string()),
            isActive: self.is_active,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserDto {
    pub email: Option<String>,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub role: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct UserFilterDto {
    pub email: Option<String>,
    pub username: Option<String>,
    pub is_active: Option<i64>,
    pub role: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

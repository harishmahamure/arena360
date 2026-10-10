use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Display-only game catalog entry (DRAFT-0022). Stores branding asset URLs that
/// the kiosk renders; launching stays client-side (ADR-0019).
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: Uuid,
    pub name: String,
    pub thumbnail_url: Option<String>,
    pub logo_url: Option<String>,
    pub video_url: Option<String>,
    pub launch_ref: Option<String>,
    pub is_active: bool,
    pub sort_order: i32,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateGameDto {
    pub name: String,
    pub thumbnail_url: Option<String>,
    pub logo_url: Option<String>,
    pub video_url: Option<String>,
    pub launch_ref: Option<String>,
    pub is_active: Option<bool>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGameDto {
    pub name: Option<String>,
    pub thumbnail_url: Option<String>,
    pub logo_url: Option<String>,
    pub video_url: Option<String>,
    pub launch_ref: Option<String>,
    pub is_active: Option<bool>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct GameFilterDto {
    pub is_active: Option<bool>,
    pub name: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

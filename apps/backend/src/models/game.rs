use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, Serializer};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

fn serialize_tags<S>(value: &Option<String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(s) if !s.is_empty() => {
            let tags: Vec<&str> = s.split(',').map(str::trim).filter(|t| !t.is_empty()).collect();
            tags.serialize(serializer)
        }
        _ => serializer.serialize_none(),
    }
}

pub fn tags_to_db(tags: &Option<Vec<String>>) -> Option<String> {
    tags.as_ref().map(|items| {
        items
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(",")
    })
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub is_active: bool,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    pub trailer_url: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub release_date: Option<DateTime<Utc>>,
    pub platform: Option<String>,
    pub category: Option<String>,
    pub is_multiplayer: bool,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub background_url: Option<String>,
    pub logo_url: Option<String>,
    #[serde(serialize_with = "serialize_tags")]
    pub tags: Option<String>,
    pub age_rating: Option<String>,
    pub min_players: Option<i32>,
    pub max_players: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateGameDto {
    pub title: String,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub is_active: Option<bool>,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    pub trailer_url: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub release_date: Option<DateTime<Utc>>,
    pub platform: Option<String>,
    pub category: Option<String>,
    pub is_multiplayer: Option<bool>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub background_url: Option<String>,
    pub logo_url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub age_rating: Option<String>,
    pub min_players: Option<i32>,
    pub max_players: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGameDto {
    pub title: Option<String>,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub is_active: Option<bool>,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    pub trailer_url: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub release_date: Option<DateTime<Utc>>,
    pub platform: Option<String>,
    pub category: Option<String>,
    pub is_multiplayer: Option<bool>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub background_url: Option<String>,
    pub logo_url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub age_rating: Option<String>,
    pub min_players: Option<i32>,
    pub max_players: Option<i32>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct GameFilterDto {
    pub genre: Option<String>,
    pub is_active: Option<i64>,
    pub title: Option<String>,
    pub platform: Option<String>,
    pub category: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub release_date: Option<String>,
    pub is_multiplayer: Option<i64>,
    pub tag: Option<String>,
    pub age_rating: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

impl GameFilterDto {
    pub fn is_active_bool(&self) -> Option<bool> {
        self.is_active.map(|v| v != 0)
    }

    pub fn is_multiplayer_bool(&self) -> Option<bool> {
        self.is_multiplayer.map(|v| v != 0)
    }
}

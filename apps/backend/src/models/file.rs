use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileRecord {
    pub id: Uuid,
    pub file_name: String,
    pub original_file_name: String,
    pub storage_key: String,
    pub bucket: Option<String>,
    pub content_type: String,
    pub file_size: i64,
    pub extension: Option<String>,
    pub category: String,
    pub status: String,
    pub visibility: String,
    pub storage_type: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration: Option<i32>,
    pub etag: Option<String>,
    pub metadata: Option<Value>,
    pub description: Option<String>,
    pub tags: Option<Value>,
    pub uploaded_by: Option<Uuid>,
    pub related_entity_type: Option<String>,
    pub related_entity_id: Option<Uuid>,
    pub download_count: i32,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateFileDto {
    pub file_name: String,
    pub original_file_name: String,
    pub storage_key: String,
    pub bucket: Option<String>,
    pub content_type: String,
    pub file_size: i64,
    pub extension: Option<String>,
    pub category: Option<String>,
    pub visibility: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration: Option<i32>,
    pub etag: Option<String>,
    pub metadata: Option<Value>,
    pub description: Option<String>,
    pub tags: Option<Value>,
    pub related_entity_type: Option<String>,
    pub related_entity_id: Option<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFileDto {
    pub file_name: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub visibility: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Value>,
    pub metadata: Option<Value>,
    pub related_entity_type: Option<String>,
    pub related_entity_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileFilterDto {
    pub category: Option<String>,
    pub status: Option<String>,
    pub uploaded_by: Option<Uuid>,
    pub content_type: Option<String>,
    pub related_entity_type: Option<String>,
    pub related_entity_id: Option<Uuid>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileWithDownloadUrlDto {
    #[serde(flatten)]
    pub file: FileRecord,
    pub download_url: String,
    pub url_expires_in: u64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatsDto {
    pub total_files: i64,
    pub total_size: i64,
    pub by_category: Value,
    pub by_status: Value,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenerateUploadUrlDto {
    pub key: String,
    pub expires_in: Option<u64>,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenerateDownloadUrlDto {
    pub key: String,
    pub expires_in: Option<u64>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListObjectsQuery {
    pub prefix: Option<String>,
    pub max_keys: Option<i32>,
    pub continuation_token: Option<String>,
}

use std::time::Duration;

use aws_credential_types::Credentials;
use aws_sdk_s3::config::{Builder as S3ConfigBuilder, Region};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::types::Object;
use aws_sdk_s3::Client;
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::config::Settings;
use crate::error::AppError;

const DEFAULT_EXPIRES_SECS: u64 = 3600;
const MIN_EXPIRES_SECS: u64 = 60;
const MAX_EXPIRES_SECS: u64 = 86400;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PresignedUploadUrlResponse {
    pub url: String,
    pub key: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PresignedDownloadUrlResponse {
    pub url: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectInfo {
    pub key: String,
    pub size: i64,
    pub last_modified: DateTime<Utc>,
    pub etag: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListObjectsResponse {
    pub objects: Vec<StorageObjectInfo>,
    pub is_truncated: bool,
    pub next_continuation_token: Option<String>,
}

#[derive(Clone)]
pub struct StorageService {
    client: Client,
    bucket: String,
    is_production: bool,
}

impl StorageService {
    pub fn try_new(settings: &Settings) -> Option<Self> {
        if !settings.r2_configured() {
            return None;
        }

        let access_key = settings.r2_access_key.clone().unwrap();
        let secret_key = settings.r2_secret_key.clone().unwrap();
        let bucket = settings.r2_bucket.clone().unwrap();
        let endpoint = settings.r2_endpoint.clone().unwrap();

        let credentials = Credentials::new(access_key, secret_key, None, None, "r2");
        let config = S3ConfigBuilder::new()
            .credentials_provider(credentials)
            .region(Region::new("auto"))
            .endpoint_url(endpoint)
            .force_path_style(true)
            .build();

        let client = Client::from_conf(config);

        Some(Self {
            client,
            bucket,
            is_production: settings.is_production(),
        })
    }

    pub fn prefix_key(&self, key: &str) -> String {
        if self.is_production {
            key.to_string()
        } else {
            format!("dev/{key}")
        }
    }

    pub async fn generate_upload_url(
        &self,
        key: &str,
        content_type: Option<&str>,
        expires_in: Option<u64>,
    ) -> Result<PresignedUploadUrlResponse, AppError> {
        validate_key(key)?;
        let full_key = self.prefix_key(key);
        let expires = clamp_expires(expires_in);

        let mut builder = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .cache_control("max-age=31536000, public");

        if let Some(ct) = content_type {
            builder = builder.content_type(ct);
        }

        let presigned = builder
            .presigned(
                PresigningConfig::expires_in(Duration::from_secs(expires))
                    .map_err(|e| AppError::Internal(e.to_string()))?,
            )
            .await
            .map_err(|e| AppError::BadRequest(format!("Failed to generate upload URL: {e}")))?;

        Ok(PresignedUploadUrlResponse {
            url: presigned.uri().to_string(),
            key: full_key,
            expires_in: expires,
        })
    }

    pub async fn generate_download_url(
        &self,
        key: &str,
        expires_in: Option<u64>,
    ) -> Result<PresignedDownloadUrlResponse, AppError> {
        validate_key(key)?;
        let expires = clamp_expires(expires_in);

        let presigned = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(
                PresigningConfig::expires_in(Duration::from_secs(expires))
                    .map_err(|e| AppError::Internal(e.to_string()))?,
            )
            .await
            .map_err(|e| AppError::BadRequest(format!("Failed to generate download URL: {e}")))?;

        Ok(PresignedDownloadUrlResponse {
            url: presigned.uri().to_string(),
            expires_in: expires,
        })
    }

    pub async fn list_objects(
        &self,
        prefix: Option<&str>,
        max_keys: Option<i32>,
        continuation_token: Option<&str>,
    ) -> Result<ListObjectsResponse, AppError> {
        let max_keys = max_keys.unwrap_or(100).clamp(1, 1000);

        let mut builder = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket)
            .max_keys(max_keys);

        if let Some(prefix) = prefix.filter(|p| !p.is_empty()) {
            builder = builder.prefix(prefix);
        }
        if let Some(token) = continuation_token.filter(|t| !t.is_empty()) {
            builder = builder.continuation_token(token);
        }

        let response = builder
            .send()
            .await
            .map_err(|e| AppError::BadRequest(format!("Failed to list objects: {e}")))?;

        let objects = response
            .contents()
            .iter()
            .filter_map(object_to_info)
            .collect();

        Ok(ListObjectsResponse {
            objects,
            is_truncated: response.is_truncated().unwrap_or(false),
            next_continuation_token: response.next_continuation_token().map(str::to_string),
        })
    }
}

fn object_to_info(obj: &Object) -> Option<StorageObjectInfo> {
    let key = obj.key()?.to_string();
    let size = obj.size()?;
    let last_modified = obj.last_modified()?.secs();
    Some(StorageObjectInfo {
        key,
        size,
        last_modified: DateTime::from_timestamp(last_modified, 0)?,
        etag: obj.e_tag().map(str::to_string),
    })
}

fn validate_key(key: &str) -> Result<(), AppError> {
    if key.trim().is_empty() {
        return Err(AppError::BadRequest("Object key cannot be empty".to_string()));
    }
    if key.contains("..") || key.starts_with('/') {
        return Err(AppError::BadRequest("Invalid object key format".to_string()));
    }
    Ok(())
}

fn clamp_expires(expires_in: Option<u64>) -> u64 {
    expires_in
        .unwrap_or(DEFAULT_EXPIRES_SECS)
        .clamp(MIN_EXPIRES_SECS, MAX_EXPIRES_SECS)
}

pub fn require_storage(storage: &Option<StorageService>) -> Result<&StorageService, AppError> {
    storage
        .as_ref()
        .ok_or_else(|| AppError::Internal("R2 storage is not configured".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_prefix_when_not_production() {
        let svc = StorageService {
            client: Client::from_conf(
                S3ConfigBuilder::new()
                    .region(Region::new("auto"))
                    .endpoint_url("https://example.r2.cloudflarestorage.com")
                    .force_path_style(true)
                    .build(),
            ),
            bucket: "test".to_string(),
            is_production: false,
        };
        assert_eq!(svc.prefix_key("images/a.jpg"), "dev/images/a.jpg");
    }

    #[test]
    fn no_prefix_in_production() {
        let svc = StorageService {
            client: Client::from_conf(
                S3ConfigBuilder::new()
                    .region(Region::new("auto"))
                    .endpoint_url("https://example.r2.cloudflarestorage.com")
                    .force_path_style(true)
                    .build(),
            ),
            bucket: "test".to_string(),
            is_production: true,
        };
        assert_eq!(svc.prefix_key("images/a.jpg"), "images/a.jpg");
    }
}

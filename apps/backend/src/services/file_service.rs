use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    CreateFileDto, FileFilterDto, FileRecord, FileWithDownloadUrlDto, StorageStatsDto,
    UpdateFileDto,
};
use crate::repositories::FileRepository;
use crate::services::storage_service::{require_storage, StorageService};

pub struct FileService {
    repo: FileRepository,
    storage: Option<StorageService>,
    upload_max_size_bytes: u64,
}

impl FileService {
    pub fn new(pool: PgPool, storage: Option<StorageService>, upload_max_size_bytes: u64) -> Self {
        Self {
            repo: FileRepository::new(pool),
            storage,
            upload_max_size_bytes,
        }
    }

    pub async fn list(
        &self,
        filters: FileFilterDto,
    ) -> Result<crate::dto::PaginationResult<FileRecord>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<FileRecord, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("File with ID {id} not found")))
    }

    pub async fn create(
        &self,
        dto: CreateFileDto,
        uploaded_by: Option<Uuid>,
    ) -> Result<FileRecord, AppError> {
        self.validate_file_size(dto.file_size)?;

        if self.repo.storage_key_exists(&dto.storage_key, None).await? {
            return Err(AppError::Conflict(format!(
                "File with storage key '{}' already exists",
                dto.storage_key
            )));
        }

        self.repo.create(&dto, uploaded_by).await
    }

    pub async fn update(&self, id: Uuid, dto: UpdateFileDto) -> Result<FileRecord, AppError> {
        self.repo.update(id, &dto).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        self.repo.soft_delete(id).await
    }

    pub async fn archive(&self, id: Uuid) -> Result<FileRecord, AppError> {
        self.update(
            id,
            UpdateFileDto {
                file_name: None,
                category: None,
                status: Some("archived".to_string()),
                visibility: None,
                description: None,
                tags: None,
                metadata: None,
                related_entity_type: None,
                related_entity_id: None,
            },
        )
        .await
    }

    pub async fn activate(&self, id: Uuid) -> Result<FileRecord, AppError> {
        self.update(
            id,
            UpdateFileDto {
                file_name: None,
                category: None,
                status: Some("active".to_string()),
                visibility: None,
                description: None,
                tags: None,
                metadata: None,
                related_entity_type: None,
                related_entity_id: None,
            },
        )
        .await
    }

    pub async fn get_storage_stats(&self) -> Result<StorageStatsDto, AppError> {
        self.repo.get_storage_stats().await
    }

    pub async fn get_with_download_url(
        &self,
        id: Uuid,
        expires_in: Option<u64>,
    ) -> Result<FileWithDownloadUrlDto, AppError> {
        let file = self.get_by_id(id).await?;
        let storage = require_storage(&self.storage)?;
        let expires = expires_in.unwrap_or(3600).clamp(60, 86400);

        let download = storage
            .generate_download_url(&file.storage_key, Some(expires))
            .await?;

        self.repo.increment_download_count(id).await?;

        Ok(FileWithDownloadUrlDto {
            file,
            download_url: download.url,
            url_expires_in: download.expires_in,
        })
    }

    pub fn validate_upload_content_length(&self, content_length: Option<u64>) -> Result<(), AppError> {
        if let Some(len) = content_length {
            self.validate_file_size(len as i64)?;
        }
        Ok(())
    }

    fn validate_file_size(&self, size: i64) -> Result<(), AppError> {
        if size < 0 {
            return Err(AppError::BadRequest(
                "File size must be greater than or equal to 0".to_string(),
            ));
        }
        if size as u64 > self.upload_max_size_bytes {
            return Err(AppError::BadRequest(format!(
                "File size exceeds maximum allowed size of {} bytes",
                self.upload_max_size_bytes
            )));
        }
        Ok(())
    }
}

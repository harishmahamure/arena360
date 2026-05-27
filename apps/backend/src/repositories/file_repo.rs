use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{CreateFileDto, FileFilterDto, FileRecord, StorageStatsDto, UpdateFileDto};

pub struct FileRepository {
    pool: PgPool,
}

impl FileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id,
               "fileName" as file_name,
               "originalFileName" as original_file_name,
               "storageKey" as storage_key,
               bucket,
               "contentType" as content_type,
               "fileSize" as file_size,
               extension,
               category::text as category,
               status::text as status,
               visibility::text as visibility,
               "storageType" as storage_type,
               width,
               height,
               duration,
               etag,
               metadata,
               description,
               tags,
               "uploadedBy" as uploaded_by,
               "relatedEntityType" as related_entity_type,
               "relatedEntityId" as related_entity_id,
               "downloadCount" as download_count,
               "lastAccessedAt" as last_accessed_at,
               "expiresAt" as expires_at,
               "createdAt" as created_at,
               "updatedAt" as updated_at,
               "deletedAt" as deleted_at
        FROM files
    "#;

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<FileRecord>, AppError> {
        let query = format!("{} WHERE id = $1 AND \"deletedAt\" IS NULL", Self::SELECT);
        let file = sqlx::query_as::<_, FileRecord>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(file)
    }

    pub async fn find_by_storage_key(&self, storage_key: &str) -> Result<Option<FileRecord>, AppError> {
        let query = format!(
            "{} WHERE \"storageKey\" = $1 AND \"deletedAt\" IS NULL",
            Self::SELECT
        );
        let file = sqlx::query_as::<_, FileRecord>(&query)
            .bind(storage_key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(file)
    }

    pub async fn list(&self, filters: &FileFilterDto) -> Result<PaginationResult<FileRecord>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, \"fileName\" as file_name, \"originalFileName\" as original_file_name, \
             \"storageKey\" as storage_key, bucket, \"contentType\" as content_type, \
             \"fileSize\" as file_size, extension, category::text as category, status::text as status, \
             visibility::text as visibility, \"storageType\" as storage_type, width, height, duration, \
             etag, metadata, description, tags, \"uploadedBy\" as uploaded_by, \
             \"relatedEntityType\" as related_entity_type, \"relatedEntityId\" as related_entity_id, \
             \"downloadCount\" as download_count, \"lastAccessedAt\" as last_accessed_at, \
             \"expiresAt\" as expires_at, \"createdAt\" as created_at, \"updatedAt\" as updated_at, \
             \"deletedAt\" as deleted_at FROM files WHERE \"deletedAt\" IS NULL",
        );

        Self::apply_filters(&mut builder, filters);
        builder.push(" ORDER BY \"createdAt\" DESC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let files = builder.build_query_as::<FileRecord>().fetch_all(&self.pool).await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM files WHERE \"deletedAt\" IS NULL");
        Self::apply_filters(&mut count_builder, filters);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(files, total.0, page, limit))
    }

    fn apply_filters(builder: &mut QueryBuilder<Postgres>, filters: &FileFilterDto) {
        if let Some(category) = filters.category.clone() {
            builder.push(" AND category = ");
            builder.push_bind(category);
        }
        if let Some(status) = filters.status.clone() {
            builder.push(" AND status = ");
            builder.push_bind(status);
        }
        if let Some(uploaded_by) = filters.uploaded_by {
            builder.push(" AND \"uploadedBy\" = ");
            builder.push_bind(uploaded_by);
        }
        if let Some(content_type) = filters.content_type.clone() {
            builder.push(" AND \"contentType\" = ");
            builder.push_bind(content_type);
        }
        if let Some(entity_type) = filters.related_entity_type.clone() {
            builder.push(" AND \"relatedEntityType\" = ");
            builder.push_bind(entity_type);
        }
        if let Some(entity_id) = filters.related_entity_id {
            builder.push(" AND \"relatedEntityId\" = ");
            builder.push_bind(entity_id);
        }
    }

    pub async fn create(&self, dto: &CreateFileDto, uploaded_by: Option<Uuid>) -> Result<FileRecord, AppError> {
        let category = dto.category.as_deref().unwrap_or("other");
        let visibility = dto.visibility.as_deref().unwrap_or("private");

        let file = sqlx::query_as::<_, FileRecord>(
            r#"
            INSERT INTO files (
                id, "fileName", "originalFileName", "storageKey", bucket, "contentType",
                "fileSize", extension, category, status, visibility, "storageType",
                width, height, duration, etag, metadata, description, tags,
                "uploadedBy", "relatedEntityType", "relatedEntityId", "downloadCount",
                "expiresAt", "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7,
                $8::files_category_enum, 'active'::files_status_enum,
                $9::files_visibility_enum, 'r2', $10, $11, $12, $13, $14, $15, $16,
                $17, $18, $19, 0, $20, NOW(), NOW()
            )
            RETURNING id,
                      "fileName" as file_name,
                      "originalFileName" as original_file_name,
                      "storageKey" as storage_key,
                      bucket,
                      "contentType" as content_type,
                      "fileSize" as file_size,
                      extension,
                      category::text as category,
                      status::text as status,
                      visibility::text as visibility,
                      "storageType" as storage_type,
                      width,
                      height,
                      duration,
                      etag,
                      metadata,
                      description,
                      tags,
                      "uploadedBy" as uploaded_by,
                      "relatedEntityType" as related_entity_type,
                      "relatedEntityId" as related_entity_id,
                      "downloadCount" as download_count,
                      "lastAccessedAt" as last_accessed_at,
                      "expiresAt" as expires_at,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(&dto.file_name)
        .bind(&dto.original_file_name)
        .bind(&dto.storage_key)
        .bind(&dto.bucket)
        .bind(&dto.content_type)
        .bind(dto.file_size)
        .bind(&dto.extension)
        .bind(category)
        .bind(visibility)
        .bind(dto.width)
        .bind(dto.height)
        .bind(dto.duration)
        .bind(&dto.etag)
        .bind(&dto.metadata)
        .bind(&dto.description)
        .bind(&dto.tags)
        .bind(uploaded_by)
        .bind(&dto.related_entity_type)
        .bind(dto.related_entity_id)
        .bind(dto.expires_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(file)
    }

    pub async fn update(&self, id: Uuid, dto: &UpdateFileDto) -> Result<FileRecord, AppError> {
        let file = sqlx::query_as::<_, FileRecord>(
            r#"
            UPDATE files SET
                "fileName" = COALESCE($2, "fileName"),
                category = COALESCE($3::files_category_enum, category),
                status = COALESCE($4::files_status_enum, status),
                visibility = COALESCE($5::files_visibility_enum, visibility),
                description = COALESCE($6, description),
                tags = COALESCE($7, tags),
                metadata = COALESCE($8, metadata),
                "relatedEntityType" = COALESCE($9, "relatedEntityType"),
                "relatedEntityId" = COALESCE($10, "relatedEntityId"),
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id,
                      "fileName" as file_name,
                      "originalFileName" as original_file_name,
                      "storageKey" as storage_key,
                      bucket,
                      "contentType" as content_type,
                      "fileSize" as file_size,
                      extension,
                      category::text as category,
                      status::text as status,
                      visibility::text as visibility,
                      "storageType" as storage_type,
                      width,
                      height,
                      duration,
                      etag,
                      metadata,
                      description,
                      tags,
                      "uploadedBy" as uploaded_by,
                      "relatedEntityType" as related_entity_type,
                      "relatedEntityId" as related_entity_id,
                      "downloadCount" as download_count,
                      "lastAccessedAt" as last_accessed_at,
                      "expiresAt" as expires_at,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .bind(&dto.file_name)
        .bind(&dto.category)
        .bind(&dto.status)
        .bind(&dto.visibility)
        .bind(&dto.description)
        .bind(&dto.tags)
        .bind(&dto.metadata)
        .bind(&dto.related_entity_type)
        .bind(dto.related_entity_id)
        .fetch_optional(&self.pool)
        .await?;

        file.ok_or_else(|| AppError::NotFound(format!("File with ID {id} not found")))
    }

    pub async fn soft_delete(&self, id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query(
            r#"
            UPDATE files
            SET "deletedAt" = NOW(), status = 'deleted'::files_status_enum, "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("File with ID {id} not found")));
        }
        Ok(())
    }

    pub async fn increment_download_count(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE files
            SET "downloadCount" = "downloadCount" + 1,
                "lastAccessedAt" = NOW(),
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_storage_stats(&self) -> Result<StorageStatsDto, AppError> {
        let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
            r#"
            SELECT category::text, status::text, COUNT(*)::bigint, COALESCE(SUM("fileSize"), 0)::bigint
            FROM files
            WHERE "deletedAt" IS NULL
            GROUP BY category, status
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut total_files: i64 = 0;
        let mut total_size: i64 = 0;
        let mut by_category = serde_json::Map::new();
        let mut by_status = serde_json::Map::new();

        for (category, status, count, size) in rows {
            total_files += count;
            total_size += size;

            let cat_entry = by_category
                .entry(category.clone())
                .or_insert(json!({ "count": 0i64, "size": 0i64 }));
            if let Some(obj) = cat_entry.as_object_mut() {
                obj.insert(
                    "count".to_string(),
                    json!(obj.get("count").and_then(|v| v.as_i64()).unwrap_or(0) + count),
                );
                obj.insert(
                    "size".to_string(),
                    json!(obj.get("size").and_then(|v| v.as_i64()).unwrap_or(0) + size),
                );
            }

            let status_count = by_status.entry(status).or_insert(json!(0i64));
            let current = status_count.as_i64().unwrap_or(0);
            *status_count = json!(current + count);
        }

        Ok(StorageStatsDto {
            total_files,
            total_size,
            by_category: Value::Object(by_category),
            by_status: Value::Object(by_status),
        })
    }

    pub async fn storage_key_exists(&self, storage_key: &str, exclude_id: Option<Uuid>) -> Result<bool, AppError> {
        let exists: (bool,) = match exclude_id {
            Some(id) => {
                sqlx::query_as(
                    r#"SELECT EXISTS(
                        SELECT 1 FROM files
                        WHERE "storageKey" = $1 AND id != $2 AND "deletedAt" IS NULL
                    )"#,
                )
                .bind(storage_key)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as(
                    r#"SELECT EXISTS(
                        SELECT 1 FROM files WHERE "storageKey" = $1 AND "deletedAt" IS NULL
                    )"#,
                )
                .bind(storage_key)
                .fetch_one(&self.pool)
                .await?
            }
        };
        Ok(exists.0)
    }
}

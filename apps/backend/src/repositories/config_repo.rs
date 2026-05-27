use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{ConfigFilterDto, Configuration, UpsertConfigDto};

pub struct ConfigRepository {
    pool: PgPool,
}

impl ConfigRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_all(
        &self,
        filters: &ConfigFilterDto,
    ) -> Result<Vec<Configuration>, AppError> {
        let mut query = String::from(
            r#"SELECT id, key, value, category, description,
                      "createdBy" as created_by, "updatedBy" as updated_by,
                      "createdAt" as created_at, "updatedAt" as updated_at
               FROM configurations WHERE 1=1"#,
        );

        let mut args: Vec<String> = Vec::new();

        if let Some(category) = &filters.category {
            args.push(category.clone());
            query.push_str(&format!(" AND category = ${}", args.len()));
        }

        if let Some(key) = &filters.key {
            args.push(format!("%{key}%"));
            query.push_str(&format!(" AND key ILIKE ${}", args.len()));
        }

        query.push_str(r#" ORDER BY category, key"#);

        let mut q = sqlx::query_as::<_, Configuration>(&query);
        for arg in &args {
            q = q.bind(arg);
        }

        let configs = q.fetch_all(&self.pool).await?;
        Ok(configs)
    }

    pub async fn find_by_key(&self, key: &str) -> Result<Option<Configuration>, AppError> {
        let config = sqlx::query_as::<_, Configuration>(
            r#"SELECT id, key, value, category, description,
                      "createdBy" as created_by, "updatedBy" as updated_by,
                      "createdAt" as created_at, "updatedAt" as updated_at
               FROM configurations WHERE key = $1"#,
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(config)
    }

    pub async fn upsert(
        &self,
        key: &str,
        category: &str,
        dto: &UpsertConfigDto,
        actor_id: Uuid,
    ) -> Result<Configuration, AppError> {
        let config = sqlx::query_as::<_, Configuration>(
            r#"INSERT INTO configurations (key, value, category, description, "createdBy", "updatedBy")
               VALUES ($1, $2, $3, $4, $5, $5)
               ON CONFLICT (key) DO UPDATE SET
                   value = EXCLUDED.value,
                   description = COALESCE(EXCLUDED.description, configurations.description),
                   "updatedBy" = $5,
                   "updatedAt" = NOW()
               RETURNING id, key, value, category, description,
                         "createdBy" as created_by, "updatedBy" as updated_by,
                         "createdAt" as created_at, "updatedAt" as updated_at"#,
        )
        .bind(key)
        .bind(&dto.value)
        .bind(category)
        .bind(&dto.description)
        .bind(actor_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(config)
    }

    pub async fn delete_by_key(&self, key: &str) -> Result<bool, AppError> {
        let result = sqlx::query("DELETE FROM configurations WHERE key = $1")
            .bind(key)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

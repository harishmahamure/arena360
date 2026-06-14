use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

pub struct SsoTokenRow {
    pub id: Uuid,
    pub purpose: String,
    pub device_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub expires_at: DateTime<Utc>,
    pub redeemed_at: Option<DateTime<Utc>>,
}

pub struct SsoRepository {
    pool: PgPool,
}

impl SsoRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert(
        &self,
        token_hash: &str,
        purpose: &str,
        device_id: Option<Uuid>,
        created_by: Uuid,
        expires_at: DateTime<Utc>,
    ) -> Result<Uuid, AppError> {
        let row: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO sso_tokens (token_hash, purpose, device_id, created_by, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
        )
        .bind(token_hash)
        .bind(purpose)
        .bind(device_id)
        .bind(created_by)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    pub async fn find_valid_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<SsoTokenRow>, AppError> {
        let row = sqlx::query_as::<_, (Uuid, String, Option<Uuid>, Option<Uuid>, DateTime<Utc>, Option<DateTime<Utc>>)>(
            r#"
            SELECT id, purpose, device_id, created_by, expires_at, redeemed_at
            FROM sso_tokens
            WHERE token_hash = $1
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(
            |(id, purpose, device_id, created_by, expires_at, redeemed_at)| SsoTokenRow {
                id,
                purpose,
                device_id,
                created_by,
                expires_at,
                redeemed_at,
            },
        ))
    }

    pub async fn mark_redeemed(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            r#"UPDATE sso_tokens SET redeemed_at = NOW() WHERE id = $1 AND redeemed_at IS NULL"#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct OutboxRow {
    pub id: i64,
    pub channel: String,
    pub event_type: String,
    pub payload: Value,
    pub audience_role: Option<String>,
    pub audience_user_id: Option<Uuid>,
    pub audience_room_id: Option<Uuid>,
    pub durable: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct OutboxService {
    pool: PgPool,
}

impl OutboxService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Publish an event into the outbox within an existing transaction.
    /// The pg_notify trigger fires on commit, waking the dispatcher.
    pub async fn publish_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        channel: &str,
        event_type: &str,
        payload: Value,
        audience_role: Option<&str>,
        audience_user_id: Option<Uuid>,
        audience_room_id: Option<Uuid>,
        durable: bool,
    ) -> Result<i64, AppError> {
        let row: (i64,) = sqlx::query_as(
            r#"INSERT INTO realtime_outbox
               (channel, event_type, payload, audience_role, audience_user_id, audience_room_id, durable)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id"#,
        )
        .bind(channel)
        .bind(event_type)
        .bind(&payload)
        .bind(audience_role)
        .bind(audience_user_id)
        .bind(audience_room_id)
        .bind(durable)
        .fetch_one(&mut **tx)
        .await?;
        Ok(row.0)
    }

    /// Fire-and-forget publish outside of a caller-managed transaction.
    /// Opens its own short transaction so the trigger fires on commit.
    pub async fn publish(
        &self,
        channel: &str,
        event_type: &str,
        payload: Value,
        audience_role: Option<&str>,
        audience_user_id: Option<Uuid>,
        durable: bool,
    ) -> Result<i64, AppError> {
        let mut tx = self.pool.begin().await?;
        let id = Self::publish_in_tx(
            &mut tx,
            channel,
            event_type,
            payload,
            audience_role,
            audience_user_id,
            None,
            durable,
        )
        .await?;
        tx.commit().await?;
        Ok(id)
    }

    pub async fn fetch_row(pool: &PgPool, id: i64) -> Result<Option<OutboxRow>, AppError> {
        let row = sqlx::query_as::<
            _,
            (
                i64,
                String,
                String,
                Value,
                Option<String>,
                Option<Uuid>,
                Option<Uuid>,
                bool,
                chrono::DateTime<chrono::Utc>,
            ),
        >(
            r#"SELECT id, channel, event_type, payload,
                      audience_role, audience_user_id, audience_room_id,
                      durable, created_at
               FROM realtime_outbox WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| OutboxRow {
            id: r.0,
            channel: r.1,
            event_type: r.2,
            payload: r.3,
            audience_role: r.4,
            audience_user_id: r.5,
            audience_room_id: r.6,
            durable: r.7,
            created_at: r.8,
        }))
    }
}

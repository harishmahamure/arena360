use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

use super::outbox::OutboxRow;

pub struct DeliveryService;

impl DeliveryService {
    pub async fn insert_delivery(
        pool: &PgPool,
        outbox_id: i64,
        subscriber_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"INSERT INTO realtime_deliveries (outbox_id, subscriber_id, delivered_at)
               VALUES ($1, $2, $3)
               ON CONFLICT (outbox_id, subscriber_id) DO NOTHING"#,
        )
        .bind(outbox_id)
        .bind(subscriber_id)
        .bind(Utc::now())
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn mark_acked(
        pool: &PgPool,
        outbox_id: i64,
        subscriber_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"UPDATE realtime_deliveries SET ack_at = $1
               WHERE outbox_id = $2 AND subscriber_id = $3 AND ack_at IS NULL"#,
        )
        .bind(Utc::now())
        .bind(outbox_id)
        .bind(subscriber_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Replay all unacked durable messages for a subscriber, ordered by outbox_id.
    pub async fn replay_pending(
        pool: &PgPool,
        subscriber_id: Uuid,
    ) -> Result<Vec<OutboxRow>, AppError> {
        let rows = sqlx::query_as::<_, (
            i64,
            String,
            String,
            serde_json::Value,
            Option<String>,
            Option<Uuid>,
            Option<Uuid>,
            bool,
            chrono::DateTime<chrono::Utc>,
        )>(
            r#"SELECT o.id, o.channel, o.event_type, o.payload,
                      o.audience_role, o.audience_user_id, o.audience_room_id,
                      o.durable, o.created_at
               FROM realtime_deliveries d
               JOIN realtime_outbox o ON o.id = d.outbox_id
               WHERE d.subscriber_id = $1 AND d.ack_at IS NULL
               ORDER BY o.id ASC
               LIMIT 500"#,
        )
        .bind(subscriber_id)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| OutboxRow {
                id: r.0,
                channel: r.1,
                event_type: r.2,
                payload: r.3,
                audience_role: r.4,
                audience_user_id: r.5,
                audience_room_id: r.6,
                durable: r.7,
                created_at: r.8,
            })
            .collect())
    }

    /// Retention: remove old rows.
    pub async fn cleanup(pool: &PgPool, acked_days: i64, unacked_days: i64) -> Result<u64, AppError> {
        let acked_cutoff = Utc::now() - chrono::Duration::days(acked_days);
        let unacked_cutoff = Utc::now() - chrono::Duration::days(unacked_days);

        let r1 = sqlx::query(
            r#"DELETE FROM realtime_deliveries WHERE ack_at IS NOT NULL AND ack_at < $1"#,
        )
        .bind(acked_cutoff)
        .execute(pool)
        .await?;

        let r2 = sqlx::query(
            r#"DELETE FROM realtime_deliveries
               WHERE ack_at IS NULL AND delivered_at IS NOT NULL AND delivered_at < $1"#,
        )
        .bind(unacked_cutoff)
        .execute(pool)
        .await?;

        let r3 = sqlx::query(
            r#"DELETE FROM realtime_outbox o
               WHERE NOT EXISTS (
                   SELECT 1 FROM realtime_deliveries d WHERE d.outbox_id = o.id
               ) AND o.created_at < $1"#,
        )
        .bind(acked_cutoff)
        .execute(pool)
        .await?;

        Ok(r1.rows_affected() + r2.rows_affected() + r3.rows_affected())
    }
}

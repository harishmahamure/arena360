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
        let rows = sqlx::query_as::<
            _,
            (
                i64,
                String,
                String,
                serde_json::Value,
                Option<String>,
                Option<Uuid>,
                Option<Uuid>,
                bool,
                chrono::DateTime<chrono::Utc>,
            ),
        >(
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

    /// Keep a rolling outbox window. Delivery rows are removed by ON DELETE CASCADE.
    pub async fn cleanup(pool: &PgPool, retention_days: i64) -> Result<u64, AppError> {
        let cutoff = Utc::now() - chrono::Duration::days(retention_days);
        let disallowed_notifications = sqlx::query(
            r#"DELETE FROM realtime_outbox o
               WHERE o.event_type = 'notification.created'
                 AND (
                     o.payload->>'kind' IS DISTINCT FROM 'kiosk_order_placed'
                     OR NOT EXISTS (
                         SELECT 1 FROM users u
                         WHERE u.id = o.audience_user_id
                           AND u.role = 'staff'
                           AND u."isActive" = true
                           AND u."deletedAt" IS NULL
                     )
                 )"#,
        )
        .execute(pool)
        .await?;

        let expired = sqlx::query(r#"DELETE FROM realtime_outbox WHERE created_at < $1"#)
            .bind(cutoff)
            .execute(pool)
            .await?;

        Ok(disallowed_notifications.rows_affected() + expired.rows_affected())
    }
}

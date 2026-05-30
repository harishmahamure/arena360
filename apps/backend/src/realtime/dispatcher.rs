use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::connection::Connection;
use super::deliveries::DeliveryService;
use super::frame::ServerFrame;
use super::outbox::OutboxService;

pub struct Dispatcher {
    pool: PgPool,
    connections: Arc<RwLock<Vec<Arc<RwLock<Connection>>>>>,
}

impl Dispatcher {
    pub fn new(pool: PgPool, connections: Arc<RwLock<Vec<Arc<RwLock<Connection>>>>>) -> Self {
        Self { pool, connections }
    }

    /// Start the PgListener loop. Runs forever; call from `tokio::spawn`.
    pub async fn run(self) {
        let mut listener = match sqlx::postgres::PgListener::connect_with(&self.pool).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Failed to connect PgListener: {e}");
                return;
            }
        };

        if let Err(e) = listener.listen("realtime_outbox_new").await {
            tracing::error!("Failed to listen on realtime_outbox_new: {e}");
            return;
        }

        tracing::info!("Realtime dispatcher listening for outbox events");

        // Also run periodic retention cleanup
        let pool_for_cleanup = self.pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
                match DeliveryService::cleanup(&pool_for_cleanup, 7, 30).await {
                    Ok(count) if count > 0 => {
                        tracing::info!("Realtime retention cleanup: removed {count} rows");
                    }
                    Err(e) => {
                        tracing::warn!("Realtime retention cleanup failed: {e}");
                    }
                    _ => {}
                }
            }
        });

        loop {
            match listener.recv().await {
                Ok(notification) => {
                    let outbox_id: i64 = match notification.payload().parse() {
                        Ok(id) => id,
                        Err(_) => continue,
                    };
                    self.dispatch(outbox_id).await;
                }
                Err(e) => {
                    tracing::warn!("PgListener error, will reconnect: {e}");
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        }
    }

    async fn dispatch(&self, outbox_id: i64) {
        let row = match OutboxService::fetch_row(&self.pool, outbox_id).await {
            Ok(Some(r)) => r,
            Ok(None) => return,
            Err(e) => {
                tracing::warn!("Failed to fetch outbox row {outbox_id}: {e}");
                return;
            }
        };

        let event_frame = ServerFrame::Event {
            msg_id: row.id,
            channel: row.channel.clone(),
            event_type: row.event_type.clone(),
            payload: row.payload.clone(),
            ts: row.created_at,
        };

        let conns = self.connections.read().await;

        for conn_lock in conns.iter() {
            let conn = conn_lock.read().await;

            if !conn.matches_channel(&row.channel) {
                continue;
            }

            if !self.passes_audience_filter(&conn, &row) {
                continue;
            }

            if row.durable {
                let _ = DeliveryService::insert_delivery(&self.pool, row.id, conn.user_id).await;
            }

            let _ = conn.outgoing_tx.send(event_frame.clone());
        }
    }

    fn passes_audience_filter(&self, conn: &Connection, row: &super::outbox::OutboxRow) -> bool {
        if let Some(ref role) = row.audience_role {
            let has_role = conn.roles.iter().any(|r| r == role);
            if !has_role {
                return false;
            }
        }

        if let Some(target_user) = row.audience_user_id {
            if conn.user_id != target_user {
                return false;
            }
        }

        true
    }
}

/// Compute which user IDs should receive a room-targeted event.
/// Not used in hot path — rooms go through channel subscription matching.
pub async fn _room_members(
    pool: &PgPool,
    room_id: Uuid,
) -> Result<Vec<Uuid>, crate::error::AppError> {
    let rows: Vec<(Uuid,)> =
        sqlx::query_as(r#"SELECT user_id FROM realtime_room_members WHERE room_id = $1"#)
            .bind(room_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

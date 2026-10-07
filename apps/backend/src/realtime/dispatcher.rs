use super::{
    frame::ServerFrame,
    registry::ConnectionRegistry,
    tenant_transport::{can_receive, TenantTransport},
    wake::RealtimeHub,
};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

pub struct Dispatcher {
    registry: Arc<ConnectionRegistry>,
    hub: RealtimeHub,
    tenant_dbs: Option<Arc<crate::tenancy::TenantDbManager>>,
    metrics: Arc<crate::metrics::Metrics>,
}
impl Dispatcher {
    pub fn new(
        registry: Arc<ConnectionRegistry>,
        hub: RealtimeHub,
        tenant_dbs: Option<Arc<crate::tenancy::TenantDbManager>>,
        metrics: Arc<crate::metrics::Metrics>,
    ) -> Self {
        Self {
            registry,
            hub,
            tenant_dbs,
            metrics,
        }
    }
    pub async fn run(self) {
        let mut receiver = self.hub.subscribe();
        let mut retry = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut cleanup = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            tokio::select! {
                received=receiver.recv()=>match received {
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_))=>self.drain_pending().await,
                    Err(broadcast::error::RecvError::Closed)=>return,
                },
                _=retry.tick()=>self.drain_pending().await,
                _=cleanup.tick()=>{
                    if let Some(manager)=&self.tenant_dbs {
                        for db in manager.open_handles().await {
                            if let Err(error)=TenantTransport::new(db).cleanup(7).await {tracing::warn!(%error,"Tenant realtime cleanup delayed");}
                        }
                    }
                }
            }
        }
    }
    async fn drain_pending(&self) {
        let Some(manager) = &self.tenant_dbs else {
            return;
        };
        let (legacy, wakes) = self.hub.pending_snapshot();
        // All live transport now comes from committed tenant events.
        for id in legacy {
            self.hub.complete_postgres(id);
        }
        for (tenant, _) in &wakes {
            if let Err(error) = manager.open(*tenant).await {
                tracing::warn!(%tenant,%error,"Tenant realtime owner unavailable");
            }
        }
        for db in manager.open_handles().await {
            let tenant = db.tenant_id();
            let transport = TenantTransport::new(db.clone());
            if let Err(error) = transport.project_pending().await {
                tracing::warn!(%tenant,%error,"Tenant realtime projection delayed");
                continue;
            }
            // Persistent projection state, rather than wake receipt, defines completion.
            if let Ok(pool) = db.background_read_pool() {
                if let Ok(cursor) = sqlx::query_scalar::<_, i64>(
                    "SELECT sequence FROM realtime_projection_cursor WHERE singleton=1",
                )
                .fetch_one(&pool)
                .await
                {
                    if let Some((_, sequences)) = wakes.iter().find(|v| v.0 == tenant) {
                        for sequence in sequences.iter().filter(|s| **s <= cursor) {
                            self.hub.complete_tenant(tenant, *sequence);
                        }
                    }
                }
            }
            let rows = match transport.pending().await {
                Ok(rows) => rows,
                Err(error) => {
                    tracing::warn!(%tenant,%error,"Tenant realtime read delayed");
                    continue;
                }
            };
            for row in rows {
                let mut recipients = Vec::new();
                let mut users = Vec::new();
                let mut verification_failed = false;
                for conn in self
                    .registry
                    .subscribers_for_tenant(tenant, &row.event.channel)
                    .await
                {
                    let conn = conn.read().await;
                    match can_receive(db.clone(), &conn.claims, &row).await {
                        Ok(true) => {}
                        Ok(false) => continue,
                        Err(error) => {
                            tracing::warn!(%tenant,%error,"Tenant delivery authorization delayed");
                            verification_failed = true;
                            break;
                        }
                    }
                    if row.event.durable {
                        users.push(conn.user_id);
                    }
                    recipients.push((conn.outgoing_tx.clone(), conn.slow_consumer.clone()));
                }
                if verification_failed {
                    break;
                }
                if row.event.durable {
                    if let Err(error) = transport.record_deliveries(row.event.id, users).await {
                        tracing::warn!(%tenant,%error,"Tenant durable delivery delayed");
                        break;
                    }
                }
                if let Err(error) = transport.mark_dispatched(row.event.id).await {
                    tracing::warn!(%tenant,%error,"Tenant delivery checkpoint delayed");
                    break;
                }
                self.metrics.set_outbox_lag(
                    Utc::now()
                        .signed_duration_since(row.event.created_at)
                        .num_milliseconds()
                        .max(0) as u64,
                );
                let frame = ServerFrame::Event {
                    msg_id: row.event.id,
                    channel: row.event.channel,
                    event_type: row.event.event_type,
                    payload: row.event.payload,
                    ts: row.event.created_at,
                };
                for (sender, slow) in recipients {
                    if matches!(
                        sender.try_send(frame.clone()),
                        Err(tokio::sync::mpsc::error::TrySendError::Full(_))
                    ) {
                        slow.notify_one();
                        self.metrics.slow_consumer_dropped();
                    }
                }
            }
        }
    }
}
use chrono::Utc;

pub(crate) struct TenantProjection {
    pub(crate) channel: String,
    pub(crate) event_type: Option<String>,
    pub(crate) payload: serde_json::Value,
    pub(crate) audience_role: Option<String>,
    pub(crate) audience_user_id: Option<Uuid>,
    pub(crate) durable: bool,
}

pub(crate) async fn tenant_projections(
    tenant_id: Uuid,
    event_type: &str,
    mut payload: serde_json::Value,
    read_pool: Option<sqlx::SqlitePool>,
) -> Result<Vec<TenantProjection>, crate::error::AppError> {
    let Some(fields) = payload.as_object_mut() else {
        return Ok(Vec::new());
    };
    fields.insert(
        "sourceTenantId".into(),
        serde_json::Value::String(tenant_id.to_string()),
    );
    let id = string_field(fields, "id");
    Ok(match event_type {
        "realtime.chat_message" => {
            let Some(channel) = string_field(fields, "channel") else {
                return Ok(Vec::new());
            };
            let Some(mut message) = fields.get("message").cloned() else {
                return Ok(Vec::new());
            };
            let Some(message_fields) = message.as_object_mut() else {
                return Ok(Vec::new());
            };
            message_fields.insert("sourceTenantId".into(), serde_json::json!(tenant_id));
            let target = match super::channel::ChannelId::parse(&channel) {
                Some(super::channel::ChannelId::User(id)) => Some(id),
                Some(super::channel::ChannelId::Room(_)) => None,
                _ => return Ok(Vec::new()),
            };
            vec![TenantProjection {
                channel,
                event_type: Some("chat.message".into()),
                payload: message,
                audience_role: None,
                audience_user_id: target,
                durable: true,
            }]
        }
        "session.started" => {
            rename_id(fields, "sessionId");
            let Some(device_id) = string_field(fields, "deviceId") else {
                return Ok(Vec::new());
            };
            vec![
                projection("staff", payload.clone(), None, true),
                projection(format!("device:{device_id}"), payload, None, false),
            ]
        }
        "session.ended" => {
            rename_id(fields, "sessionId");
            let (Some(device_id), Some(player_id)) = (
                string_field(fields, "deviceId"),
                uuid_field(fields, "playerId"),
            ) else {
                return Ok(Vec::new());
            };
            vec![
                projection("staff", payload.clone(), None, true),
                projection(format!("device:{device_id}"), payload.clone(), None, true),
                projection(format!("user:{player_id}"), payload, Some(player_id), false),
            ]
        }
        "device.status_changed" => {
            let Some(device_id) = id else {
                return Ok(Vec::new());
            };
            fields.insert(
                "deviceId".into(),
                serde_json::Value::String(device_id.clone()),
            );
            vec![
                projection("staff", payload.clone(), None, false),
                projection(format!("device:{device_id}"), payload, None, false),
            ]
        }
        "balance.updated" => {
            rename_id(fields, "balanceId");
            let Some(player_id) = uuid_field(fields, "playerId") else {
                return Ok(Vec::new());
            };
            let session_id = uuid_field(fields, "sessionId");
            let resolved = if let (Some(session_id), Some(device_id)) =
                (session_id, uuid_field(fields, "deviceId"))
            {
                Some((session_id, device_id))
            } else {
                balance_session(read_pool, player_id, session_id).await?
            };
            let Some((session_id, device_id)) = resolved else {
                return Ok(Vec::new());
            };
            fields.insert(
                "sessionId".into(),
                serde_json::Value::String(session_id.to_string()),
            );
            vec![
                projection(format!("device:{device_id}"), payload.clone(), None, false),
                projection(
                    format!("user:{player_id}"),
                    payload.clone(),
                    Some(player_id),
                    false,
                ),
                projection("staff", payload, None, false),
            ]
        }
        "configuration.changed" | "pricing.rules.changed" => {
            fields.insert(
                "organizationId".into(),
                serde_json::Value::String(tenant_id.to_string()),
            );
            vec![projection("configuration", payload, None, true)]
        }
        "access.changed" => {
            let users = fields
                .get("userIds")
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|v| v.as_str().and_then(|v| Uuid::parse_str(v).ok()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            users
                .into_iter()
                .map(|user| projection(format!("user:{user}"), payload.clone(), Some(user), true))
                .collect()
        }
        "notification.created" => {
            let Some(user) = uuid_field(fields, "userId") else {
                return Ok(Vec::new());
            };
            vec![projection(
                format!("user:{user}"),
                payload,
                Some(user),
                true,
            )]
        }
        "kitchen.changed" => vec![
            projection("kitchen", payload.clone(), None, true),
            projection("admin", payload.clone(), None, true),
            projection("staff", payload, None, true),
        ],
        "approval.requested" => vec![TenantProjection {
            channel: "admin".into(),
            event_type: None,
            payload,
            audience_role: Some("admin".into()),
            audience_user_id: None,
            durable: true,
        }],
        "approval.decided" => {
            let requester = uuid_field(fields, "requestedBy");
            let mut projections = vec![TenantProjection {
                channel: "admin".into(),
                event_type: None,
                payload: payload.clone(),
                audience_role: Some("admin".into()),
                audience_user_id: None,
                durable: true,
            }];
            if let Some(user) = requester {
                projections.push(projection(
                    format!("user:{user}"),
                    payload,
                    Some(user),
                    true,
                ));
            }
            projections
        }
        "kiosk_order.placed" => {
            rename_id(fields, "orderId");
            vec![TenantProjection {
                channel: "staff".into(),
                event_type: None,
                payload,
                audience_role: Some("staff".into()),
                audience_user_id: None,
                durable: false,
            }]
        }
        "transaction.created"
            if fields.get("inventoryLocationId").is_some()
                && fields
                    .get("paymentStatus")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|status| matches!(status, "completed" | "credit")) =>
        {
            let Some(transaction_id) = id else {
                return Ok(Vec::new());
            };
            let (Some(_actor_id), Some(payment_method)) = (
                uuid_field(fields, "actorId"),
                string_field(fields, "paymentMethod"),
            ) else {
                return Ok(Vec::new());
            };
            if string_field(fields, "actorRole").as_deref() != Some("staff") {
                return Ok(Vec::new());
            }
            let sale_payload = serde_json::json!({
                "transaction_id": transaction_id,
                "amount": fields.get("amount").cloned().unwrap_or(serde_json::Value::Null),
                "payment_method": payment_method,
                "transaction_type": fields.get("transactionType").cloned()
                    .unwrap_or_else(|| serde_json::Value::String("product_purchase".into())),
                "sourceTenantId": tenant_id,
            });
            vec![TenantProjection {
                channel: "admin".into(),
                event_type: Some("transaction.sale_completed".into()),
                payload: sale_payload,
                audience_role: Some("admin".into()),
                audience_user_id: None,
                durable: true,
            }]
        }
        _ => Vec::new(),
    })
}

fn projection(
    channel: impl Into<String>,
    payload: serde_json::Value,
    audience_user_id: Option<Uuid>,
    durable: bool,
) -> TenantProjection {
    TenantProjection {
        channel: channel.into(),
        event_type: None,
        payload,
        audience_role: None,
        audience_user_id,
        durable,
    }
}

fn rename_id(fields: &mut serde_json::Map<String, serde_json::Value>, target: &'static str) {
    if let Some(id) = fields.remove("id") {
        fields.insert(target.into(), id);
    }
}

fn string_field(fields: &serde_json::Map<String, serde_json::Value>, name: &str) -> Option<String> {
    fields
        .get(name)
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}

fn uuid_field(fields: &serde_json::Map<String, serde_json::Value>, name: &str) -> Option<Uuid> {
    string_field(fields, name).and_then(|id| Uuid::parse_str(&id).ok())
}

async fn balance_session(
    pool: Option<sqlx::SqlitePool>,
    player_id: Uuid,
    session_id: Option<Uuid>,
) -> Result<Option<(Uuid, Uuid)>, crate::error::AppError> {
    let Some(pool) = pool else {
        return Ok(None);
    };
    if let Some(session_id) = session_id {
        let device: Option<String> =
            sqlx::query_scalar("SELECT device_id FROM usage_sessions WHERE id = ?")
                .bind(session_id.to_string())
                .fetch_optional(&pool)
                .await?;
        return Ok(device
            .and_then(|id| Uuid::parse_str(&id).ok())
            .map(|device_id| (session_id, device_id)));
    }
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT id, device_id FROM usage_sessions
         WHERE player_id = ? AND end_time IS NULL AND deleted_at IS NULL
         ORDER BY start_time DESC LIMIT 1",
    )
    .bind(player_id.to_string())
    .fetch_optional(&pool)
    .await?;
    Ok(row.and_then(|(session, device)| {
        Some((
            Uuid::parse_str(&session).ok()?,
            Uuid::parse_str(&device).ok()?,
        ))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tenancy::{write_outbox_event_on_connection, NewOutboxEvent};
    use serde_json::json;
    async fn tenant_projections(
        tenant: Uuid,
        kind: &str,
        payload: serde_json::Value,
        pool: Option<sqlx::SqlitePool>,
    ) -> Vec<TenantProjection> {
        super::tenant_projections(tenant, kind, payload, pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn tenant_projection_preserves_channels_durability_and_allowlist() {
        let tenant_id = Uuid::new_v4();
        let device_id = Uuid::new_v4();
        let player_id = Uuid::new_v4();
        let started = tenant_projections(
            tenant_id,
            "session.started",
            json!({
                "id": Uuid::new_v4(),
                "deviceId": device_id,
                "playerId": player_id,
                "deviceName": "PC-1",
                "walletMinutesAtStart": 60,
                "remainingMinutes": 60,
                "deductionProfile": {"mode":"weighted"},
                "cafeTimezone": "UTC"
            }),
            None,
        )
        .await;
        assert_eq!(started.len(), 2);
        assert_eq!(started[0].channel, "staff");
        assert!(started[0].durable);
        assert_eq!(started[1].channel, format!("device:{device_id}"));
        assert_eq!(started[0].payload["deviceName"], "PC-1");
        assert_eq!(started[0].payload["walletMinutesAtStart"], 60);
        let session = tenant_projections(
            tenant_id,
            "session.ended",
            json!({"id":Uuid::new_v4(),"deviceId":device_id,"playerId":player_id}),
            None,
        )
        .await;
        assert_eq!(session.len(), 3);
        assert_eq!(session[0].channel, "staff");
        assert!(session[0].durable);
        assert_eq!(session[1].channel, format!("device:{device_id}"));
        assert!(session[1].durable);
        assert_eq!(session[2].audience_user_id, Some(player_id));
        assert!(!session[2].durable);

        let device = tenant_projections(
            tenant_id,
            "device.status_changed",
            json!({"id":device_id,"status":"available"}),
            None,
        )
        .await;
        assert_eq!(device.len(), 2);
        assert!(device.iter().all(|event| !event.durable));

        let balance = tenant_projections(
            tenant_id,
            "balance.updated",
            json!({
                "id": Uuid::new_v4(),
                "playerId": player_id,
                "deviceId": device_id,
                "sessionId": Uuid::new_v4(),
                "remainingMinutes": 31
            }),
            None,
        )
        .await;
        assert_eq!(balance.len(), 3);
        assert_eq!(balance[0].channel, format!("device:{device_id}"));
        assert_eq!(balance[1].audience_user_id, Some(player_id));
        assert_eq!(balance[2].channel, "staff");

        let config = tenant_projections(
            tenant_id,
            "configuration.changed",
            json!({"key":"timezone"}),
            None,
        )
        .await;
        assert_eq!(config.len(), 1);
        assert_eq!(config[0].channel, "configuration");
        assert!(config[0].durable);
        assert_eq!(config[0].payload["organizationId"], tenant_id.to_string());
        assert_eq!(config[0].payload["sourceTenantId"], tenant_id.to_string());
        let pricing = tenant_projections(
            tenant_id,
            "pricing.rules.changed",
            json!({"ruleSetId":Uuid::new_v4(),"version":3}),
            None,
        )
        .await;
        assert_eq!(pricing.len(), 1);
        assert_eq!(pricing[0].channel, "configuration");
        assert!(pricing[0].durable);

        let kiosk = tenant_projections(
            tenant_id,
            "kiosk_order.placed",
            json!({
                "id": Uuid::new_v4(),
                "deviceId": device_id,
                "deviceName": "PC-1",
                "playerUsername": "player",
                "items": [{"productName":"Cola","quantity":2,"unitPrice":12.5}]
            }),
            None,
        )
        .await;
        assert_eq!(kiosk.len(), 1);
        assert_eq!(kiosk[0].channel, "staff");
        assert_eq!(kiosk[0].audience_role.as_deref(), Some("staff"));
        assert!(!kiosk[0].durable);
        assert_eq!(kiosk[0].payload["items"][0]["unitPrice"], 12.5);

        let staff_id = Uuid::new_v4();
        let staff_pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE users(
                id TEXT PRIMARY KEY, role TEXT NOT NULL,
                is_active INTEGER NOT NULL, deleted_at TEXT
            )",
        )
        .execute(&staff_pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO users(id,role,is_active) VALUES(?,'staff',1)")
            .bind(staff_id.to_string())
            .execute(&staff_pool)
            .await
            .unwrap();
        let admin_id = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,role,is_active) VALUES(?,'admin',1)")
            .bind(admin_id.to_string())
            .execute(&staff_pool)
            .await
            .unwrap();
        // Projection must use the committed actorRole snapshot, not this mutable row.
        sqlx::query("UPDATE users SET role='admin',is_active=0 WHERE id=?")
            .bind(staff_id.to_string())
            .execute(&staff_pool)
            .await
            .unwrap();
        let sale = tenant_projections(
            tenant_id,
            "transaction.created",
            json!({
                "id": Uuid::new_v4(),
                "amount": 42.5,
                "paymentStatus": "completed",
                "paymentMethod": "cash",
                "actorId": staff_id,
                "actorRole": "staff",
                "transactionType": "product_purchase",
                "inventoryLocationId": Uuid::new_v4()
            }),
            Some(staff_pool.clone()),
        )
        .await;
        assert_eq!(sale.len(), 1);
        assert_eq!(sale[0].channel, "admin");
        assert_eq!(
            sale[0].event_type.as_deref(),
            Some("transaction.sale_completed")
        );
        assert_eq!(sale[0].audience_role.as_deref(), Some("admin"));
        assert!(sale[0].durable);
        assert_eq!(sale[0].payload["payment_method"], "cash");
        sqlx::query("UPDATE users SET role='staff' WHERE id=?")
            .bind(admin_id.to_string())
            .execute(&staff_pool)
            .await
            .unwrap();
        assert!(tenant_projections(
            tenant_id,
            "transaction.created",
            json!({
                "id": Uuid::new_v4(),
                "amount": 42.5,
                "paymentStatus": "completed",
                "paymentMethod": "cash",
                "actorId": admin_id,
                "actorRole": "admin",
                "inventoryLocationId": Uuid::new_v4()
            }),
            Some(staff_pool.clone()),
        )
        .await
        .is_empty());
        let player_actor_id = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,role,is_active) VALUES(?,'staff',1)")
            .bind(player_actor_id.to_string())
            .execute(&staff_pool)
            .await
            .unwrap();
        assert!(tenant_projections(
            tenant_id,
            "transaction.created",
            json!({
                "id": Uuid::new_v4(),
                "amount": 42.5,
                "paymentStatus": "credit",
                "paymentMethod": "credit",
                "actorId": player_actor_id,
                "actorRole": "player",
                "inventoryLocationId": Uuid::new_v4()
            }),
            Some(staff_pool.clone()),
        )
        .await
        .is_empty());
        assert!(tenant_projections(
            tenant_id,
            "transaction.created",
            json!({
                "id": Uuid::new_v4(),
                "amount": 42.5,
                "paymentStatus": "completed",
                "paymentMethod": "cash",
                "inventoryLocationId": Uuid::new_v4()
            }),
            Some(staff_pool),
        )
        .await
        .is_empty());

        for event_type in [
            "session.heartbeat",
            "product.updated",
            "plan.updated",
            "user.updated",
            "analytics.snapshot",
        ] {
            assert!(tenant_projections(tenant_id, event_type, json!({}), None)
                .await
                .is_empty());
        }
    }

    #[tokio::test]
    async fn committed_sqlite_outbox_row_projects_with_realtime_semantics() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE outbox_events(
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL, location_id TEXT,
                aggregate_type TEXT NOT NULL, aggregate_id TEXT NOT NULL,
                event_type TEXT NOT NULL, occurred_at TEXT NOT NULL,
                schema_version INTEGER NOT NULL, deleted INTEGER NOT NULL,
                payload TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        let tenant_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let player_id = Uuid::new_v4();
        let device_id = Uuid::new_v4();
        let mut connection = pool.acquire().await.unwrap();
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *connection)
            .await
            .unwrap();
        write_outbox_event_on_connection(
            &mut connection,
            NewOutboxEvent {
                location_id: Some(Uuid::new_v4()),
                aggregate_type: "session".into(),
                aggregate_id: session_id,
                event_type: "session.ended".into(),
                schema_version: 1,
                deleted: false,
                payload: json!({
                    "id": session_id,
                    "playerId": player_id,
                    "deviceId": device_id,
                    "remainingMinutes": 17,
                    "reason": "voluntary"
                }),
            },
        )
        .await
        .unwrap();
        sqlx::query("COMMIT")
            .execute(&mut *connection)
            .await
            .unwrap();
        drop(connection);
        let (event_type, payload): (String, String) =
            sqlx::query_as("SELECT event_type,payload FROM outbox_events WHERE sequence=1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let projected = tenant_projections(
            tenant_id,
            &event_type,
            serde_json::from_str(&payload).unwrap(),
            Some(pool),
        )
        .await;
        assert_eq!(projected.len(), 3);
        assert_eq!(projected[0].channel, "staff");
        assert!(projected[0].durable);
        assert_eq!(projected[1].channel, format!("device:{device_id}"));
        assert!(projected[1].durable);
        assert_eq!(projected[2].audience_user_id, Some(player_id));
        assert_eq!(projected[0].payload["remainingMinutes"], 17);
    }
}

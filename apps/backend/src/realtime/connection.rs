use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::acl;
use super::channel::ChannelId;
use super::deliveries::DeliveryService;
use super::frame::{ClientFrame, ServerFrame};
use super::outbox::OutboxService;
use crate::dto::JwtUserClaims;

/// Per-connection actor. Spawned once per accepted WebSocket.
pub struct Connection {
    pub user_id: Uuid,
    pub roles: Vec<String>,
    pub claims: JwtUserClaims,
    pub subscriptions: HashSet<String>,
    pub outgoing_tx: mpsc::UnboundedSender<ServerFrame>,
}

impl Connection {
    pub fn matches_channel(&self, channel: &str) -> bool {
        self.subscriptions.contains(channel)
    }
}

pub async fn run(
    socket: WebSocket,
    claims: JwtUserClaims,
    pool: PgPool,
    connections: Arc<tokio::sync::RwLock<Vec<Arc<tokio::sync::RwLock<Connection>>>>>,
    outbox: OutboxService,
) {
    let user_id = match claims.user_id_uuid() {
        Some(id) => id,
        None => return,
    };

    let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<ServerFrame>();

    let conn = Arc::new(tokio::sync::RwLock::new(Connection {
        user_id,
        roles: claims.roles.clone(),
        claims: claims.clone(),
        subscriptions: HashSet::new(),
        outgoing_tx: outgoing_tx.clone(),
    }));

    {
        let mut conns = connections.write().await;
        conns.push(conn.clone());
    }

    let welcome = ServerFrame::Welcome {
        user_id,
        roles: claims.roles.clone(),
    };
    let _ = outgoing_tx.send(welcome);

    // Replay unacked durable messages
    if let Ok(pending) = DeliveryService::replay_pending(&pool, user_id).await {
        for row in pending {
            let event = ServerFrame::Event {
                msg_id: row.id,
                channel: row.channel,
                event_type: row.event_type,
                payload: row.payload,
                ts: row.created_at,
            };
            let _ = outgoing_tx.send(event);
        }
    }

    let (mut ws_sink, mut ws_stream) = socket.split();

    // Write loop: outgoing_rx -> websocket
    let write_handle = tokio::spawn(async move {
        while let Some(frame) = outgoing_rx.recv().await {
            let json = match serde_json::to_string(&frame) {
                Ok(j) => j,
                Err(_) => continue,
            };
            if ws_sink.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    // Read loop: websocket -> process frames
    let pool_clone = pool.clone();
    let conn_clone = conn.clone();
    let read_handle = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_stream.next().await {
            match msg {
                Message::Text(text) => {
                    let frame: ClientFrame = match serde_json::from_str(&text) {
                        Ok(f) => f,
                        Err(_) => {
                            let _ = outgoing_tx
                                .send(ServerFrame::error("INVALID_FRAME", "Malformed JSON frame"));
                            continue;
                        }
                    };

                    handle_client_frame(
                        frame,
                        &conn_clone,
                        &outgoing_tx,
                        &pool_clone,
                        &outbox,
                    )
                    .await;
                }
                Message::Close(_) => break,
                Message::Ping(data) => {
                    let _ = outgoing_tx.send(ServerFrame::Pong);
                    let _ = data; // ping payload ignored
                }
                _ => {}
            }
        }
    });

    // Wait for either loop to end
    tokio::select! {
        _ = write_handle => {},
        _ = read_handle => {},
    }

    // Remove connection from the shared list
    let mut conns = connections.write().await;
    conns.retain(|c| !Arc::ptr_eq(c, &conn));
}

async fn handle_client_frame(
    frame: ClientFrame,
    conn: &Arc<tokio::sync::RwLock<Connection>>,
    tx: &mpsc::UnboundedSender<ServerFrame>,
    pool: &PgPool,
    outbox: &OutboxService,
) {
    match frame {
        ClientFrame::Subscribe { channels } => {
            let mut subscribed = Vec::new();
            let conn_read = conn.read().await;

            for ch_raw in &channels {
                let channel = match ChannelId::parse(ch_raw) {
                    Some(c) => c,
                    None => {
                        let _ = tx.send(ServerFrame::error(
                            "UNKNOWN_CHANNEL",
                            format!("Unknown channel: {ch_raw}"),
                        ));
                        continue;
                    }
                };

                if let Err(e) = acl::can_subscribe(&conn_read.claims, &channel) {
                    let _ = tx.send(ServerFrame::error("FORBIDDEN_CHANNEL", e.to_string()));
                    continue;
                }

                if let ChannelId::Room(ref name) = channel {
                    match acl::is_room_member(pool, name, conn_read.user_id).await {
                        Ok(true) => {}
                        Ok(false) => {
                            let _ = tx.send(ServerFrame::error(
                                "NOT_MEMBER",
                                format!("Not a member of room:{name}"),
                            ));
                            continue;
                        }
                        Err(_) => {
                            let _ = tx.send(ServerFrame::error(
                                "INTERNAL_ERROR",
                                "Failed to check room membership",
                            ));
                            continue;
                        }
                    }
                }

                subscribed.push(ch_raw.clone());
            }

            drop(conn_read);

            if !subscribed.is_empty() {
                let mut conn_write = conn.write().await;
                for ch in &subscribed {
                    conn_write.subscriptions.insert(ch.clone());
                }
                drop(conn_write);
                let _ = tx.send(ServerFrame::Subscribed {
                    channels: subscribed,
                });
            }
        }

        ClientFrame::Unsubscribe { channels } => {
            let mut unsubscribed = Vec::new();
            let mut conn_write = conn.write().await;
            for ch in &channels {
                if conn_write.subscriptions.remove(ch) {
                    unsubscribed.push(ch.clone());
                }
            }
            drop(conn_write);

            if !unsubscribed.is_empty() {
                let _ = tx.send(ServerFrame::Unsubscribed {
                    channels: unsubscribed,
                });
            }
        }

        ClientFrame::Ack { msg_id } => {
            let conn_read = conn.read().await;
            let _ = DeliveryService::mark_acked(pool, msg_id, conn_read.user_id).await;
        }

        ClientFrame::Publish { channel, payload } => {
            let conn_read = conn.read().await;
            let ch = match ChannelId::parse(&channel) {
                Some(c) => c,
                None => {
                    let _ = tx.send(ServerFrame::error(
                        "UNKNOWN_CHANNEL",
                        format!("Unknown channel: {channel}"),
                    ));
                    return;
                }
            };

            if let Err(e) = acl::can_publish(&conn_read.claims, &ch) {
                let _ = tx.send(ServerFrame::error("FORBIDDEN_CHANNEL", e.to_string()));
                return;
            }

            if let ChannelId::Room(ref name) = ch {
                match acl::is_room_member(pool, name, conn_read.user_id).await {
                    Ok(true) => {}
                    Ok(false) => {
                        let _ = tx.send(ServerFrame::error(
                            "NOT_MEMBER",
                            format!("Not a member of room:{name}"),
                        ));
                        return;
                    }
                    Err(_) => {
                        let _ = tx.send(ServerFrame::error(
                            "INTERNAL_ERROR",
                            "Failed to check room membership",
                        ));
                        return;
                    }
                }
            }

            if !conn_read.subscriptions.contains(&channel) {
                let _ = tx.send(ServerFrame::error(
                    "NOT_SUBSCRIBED",
                    "Subscribe to a channel before publishing",
                ));
                return;
            }

            let user_id = conn_read.user_id;
            let mut chat_payload = payload;
            chat_payload["sender_id"] = serde_json::json!(user_id.to_string());

            drop(conn_read);

            let _ = outbox
                .publish(&channel, "chat.message", chat_payload, None, None, true)
                .await;
        }

        ClientFrame::Ping => {
            let _ = tx.send(ServerFrame::Pong);
        }
    }
}

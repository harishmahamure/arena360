use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Notify};
use uuid::Uuid;

use super::acl;
use super::channel::ChannelId;
use super::deliveries::DeliveryService;
use super::frame::{ClientFrame, ServerFrame};
use super::outbox::OutboxService;
use super::registry::ConnectionRegistry;
use crate::dto::JwtUserClaims;

const OUTGOING_QUEUE_CAPACITY: usize = 256;
const MAX_FRAME_BYTES: usize = 64 * 1024;
const MAX_SUBSCRIPTIONS: usize = 32;

/// Per-connection actor. Spawned once per accepted WebSocket.
pub struct Connection {
    pub id: Uuid,
    pub user_id: Uuid,
    pub roles: Vec<String>,
    pub claims: JwtUserClaims,
    pub subscriptions: HashSet<String>,
    pub outgoing_tx: mpsc::Sender<ServerFrame>,
    pub slow_consumer: Arc<Notify>,
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
    registry: Arc<ConnectionRegistry>,
    outbox: OutboxService,
    metrics: Arc<crate::metrics::Metrics>,
) {
    let user_id = match claims.user_id_uuid() {
        Some(id) => id,
        None => return,
    };

    let connection_id = Uuid::new_v4();
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<ServerFrame>(OUTGOING_QUEUE_CAPACITY);
    let slow_consumer = Arc::new(Notify::new());

    let conn = Arc::new(tokio::sync::RwLock::new(Connection {
        id: connection_id,
        user_id,
        roles: claims.roles.clone(),
        claims: claims.clone(),
        subscriptions: HashSet::new(),
        outgoing_tx: outgoing_tx.clone(),
        slow_consumer: slow_consumer.clone(),
    }));
    registry.insert(connection_id, conn.clone()).await;
    metrics.websocket_opened();

    let (mut ws_sink, mut ws_stream) = socket.split();

    // Start the writer before enqueuing the replay. A reconnect can have more
    // pending deliveries than the bounded queue can hold, so the queue must be
    // drained while the replay is being populated.
    let write_slow_consumer = slow_consumer.clone();
    let mut write_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(frame) = outgoing_rx.recv() => {
                    if ws_sink.send(Message::Binary(frame.encode_binary().into())).await.is_err() { break; }
                }
                _ = write_slow_consumer.notified() => {
                    let _ = ws_sink.send(Message::Close(Some(axum::extract::ws::CloseFrame {
                        code: 1013,
                        reason: "slow consumer; retry later".into(),
                    }))).await;
                    break;
                }
                else => break,
            }
        }
    });

    let welcome = ServerFrame::Welcome {
        user_id,
        roles: claims.roles.clone(),
    };
    send_frame(&outgoing_tx, &slow_consumer, welcome);

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
            send_frame(&outgoing_tx, &slow_consumer, event);
        }
    }

    // Read loop: websocket -> process frames
    let pool_clone = pool.clone();
    let conn_clone = conn.clone();
    let read_registry = registry.clone();
    let read_slow_consumer = slow_consumer.clone();
    let mut read_handle = tokio::spawn(async move {
        loop {
            let next =
                match tokio::time::timeout(std::time::Duration::from_secs(90), ws_stream.next())
                    .await
                {
                    Ok(value) => value,
                    Err(_) => break,
                };
            let Some(Ok(msg)) = next else {
                break;
            };
            match msg {
                Message::Binary(bytes) if bytes.len() <= MAX_FRAME_BYTES => {
                    let frame = match ClientFrame::decode_binary(&bytes) {
                        Ok(f) => f,
                        Err(_) => {
                            send_frame(
                                &outgoing_tx,
                                &read_slow_consumer,
                                ServerFrame::error("INVALID_FRAME", "Malformed protobuf frame"),
                            );
                            continue;
                        }
                    };

                    handle_client_frame(
                        frame,
                        &conn_clone,
                        &outgoing_tx,
                        &pool_clone,
                        &outbox,
                        &read_registry,
                        &read_slow_consumer,
                    )
                    .await;
                }
                Message::Binary(_) => {
                    send_frame(
                        &outgoing_tx,
                        &read_slow_consumer,
                        ServerFrame::error("FRAME_TOO_LARGE", "Frame exceeds 64 KiB"),
                    );
                    break;
                }
                Message::Text(_) => {
                    send_frame(
                        &outgoing_tx,
                        &read_slow_consumer,
                        ServerFrame::error(
                            "BINARY_REQUIRED",
                            "Use arena360.protobuf.v1 binary frames",
                        ),
                    );
                }
                Message::Close(_) => break,
                Message::Ping(data) => {
                    send_frame(&outgoing_tx, &read_slow_consumer, ServerFrame::Pong);
                    let _ = data; // ping payload ignored
                }
                _ => {}
            }
        }
    });

    // Wait for either loop to end
    tokio::select! {
        _ = &mut write_handle => {},
        _ = &mut read_handle => {},
    }
    write_handle.abort();
    read_handle.abort();

    // Remove connection from the shared list
    registry.remove(connection_id).await;
    metrics.websocket_closed();
}

fn send_frame(tx: &mpsc::Sender<ServerFrame>, slow_consumer: &Notify, frame: ServerFrame) {
    if matches!(tx.try_send(frame), Err(mpsc::error::TrySendError::Full(_))) {
        slow_consumer.notify_one();
    }
}

async fn handle_client_frame(
    frame: ClientFrame,
    conn: &Arc<tokio::sync::RwLock<Connection>>,
    tx: &mpsc::Sender<ServerFrame>,
    pool: &PgPool,
    outbox: &OutboxService,
    registry: &ConnectionRegistry,
    slow_consumer: &Notify,
) {
    match frame {
        ClientFrame::Subscribe { channels } => {
            let mut subscribed = Vec::new();
            let conn_read = conn.read().await;

            for ch_raw in &channels {
                let channel = match ChannelId::parse(ch_raw) {
                    Some(c) => c,
                    None => {
                        send_frame(
                            tx,
                            slow_consumer,
                            ServerFrame::error(
                                "UNKNOWN_CHANNEL",
                                format!("Unknown channel: {ch_raw}"),
                            ),
                        );
                        continue;
                    }
                };

                if let Err(e) = acl::can_subscribe(&conn_read.claims, &channel) {
                    send_frame(
                        tx,
                        slow_consumer,
                        ServerFrame::error("FORBIDDEN_CHANNEL", e.to_string()),
                    );
                    continue;
                }

                if let ChannelId::User(player_id) = channel {
                    if conn_read.claims.is_device() {
                        let device_id = conn_read.user_id;
                        match acl::device_has_player_session(pool, device_id, player_id).await {
                            Ok(true) => {}
                            Ok(false) => {
                                send_frame(
                                    tx,
                                    slow_consumer,
                                    ServerFrame::error(
                                        "FORBIDDEN_CHANNEL",
                                        "No active session for player on this device".to_string(),
                                    ),
                                );
                                continue;
                            }
                            Err(_) => {
                                send_frame(
                                    tx,
                                    slow_consumer,
                                    ServerFrame::error(
                                        "INTERNAL_ERROR",
                                        "Failed to verify player session",
                                    ),
                                );
                                continue;
                            }
                        }
                    }
                }

                if let ChannelId::Room(ref name) = channel {
                    match acl::is_room_member(pool, name, conn_read.user_id).await {
                        Ok(true) => {}
                        Ok(false) => {
                            send_frame(
                                tx,
                                slow_consumer,
                                ServerFrame::error(
                                    "NOT_MEMBER",
                                    format!("Not a member of room:{name}"),
                                ),
                            );
                            continue;
                        }
                        Err(_) => {
                            send_frame(
                                tx,
                                slow_consumer,
                                ServerFrame::error(
                                    "INTERNAL_ERROR",
                                    "Failed to check room membership",
                                ),
                            );
                            continue;
                        }
                    }
                }

                subscribed.push(ch_raw.clone());
            }

            drop(conn_read);

            if !subscribed.is_empty() {
                let mut conn_write = conn.write().await;
                let available = MAX_SUBSCRIPTIONS.saturating_sub(conn_write.subscriptions.len());
                subscribed.truncate(available);
                for ch in &subscribed {
                    conn_write.subscriptions.insert(ch.clone());
                }
                let connection_id = conn_write.id;
                drop(conn_write);
                registry.subscribe(connection_id, &subscribed).await;
                send_frame(
                    tx,
                    slow_consumer,
                    ServerFrame::Subscribed {
                        channels: subscribed,
                    },
                );
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
                let connection_id = conn.read().await.id;
                registry.unsubscribe(connection_id, &unsubscribed).await;
                send_frame(
                    tx,
                    slow_consumer,
                    ServerFrame::Unsubscribed {
                        channels: unsubscribed,
                    },
                );
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
                    send_frame(
                        tx,
                        slow_consumer,
                        ServerFrame::error(
                            "UNKNOWN_CHANNEL",
                            format!("Unknown channel: {channel}"),
                        ),
                    );
                    return;
                }
            };

            if let Err(e) = acl::can_publish(&conn_read.claims, &ch) {
                send_frame(
                    tx,
                    slow_consumer,
                    ServerFrame::error("FORBIDDEN_CHANNEL", e.to_string()),
                );
                return;
            }

            if let ChannelId::Room(ref name) = ch {
                match acl::is_room_member(pool, name, conn_read.user_id).await {
                    Ok(true) => {}
                    Ok(false) => {
                        send_frame(
                            tx,
                            slow_consumer,
                            ServerFrame::error(
                                "NOT_MEMBER",
                                format!("Not a member of room:{name}"),
                            ),
                        );
                        return;
                    }
                    Err(_) => {
                        send_frame(
                            tx,
                            slow_consumer,
                            ServerFrame::error("INTERNAL_ERROR", "Failed to check room membership"),
                        );
                        return;
                    }
                }
            }

            if !conn_read.subscriptions.contains(&channel) {
                send_frame(
                    tx,
                    slow_consumer,
                    ServerFrame::error(
                        "NOT_SUBSCRIBED",
                        "Subscribe to a channel before publishing",
                    ),
                );
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
            send_frame(tx, slow_consumer, ServerFrame::Pong);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn full_outgoing_queue_notifies_slow_consumer() {
        let (tx, _rx) = mpsc::channel(1);
        let slow_consumer = Notify::new();

        send_frame(&tx, &slow_consumer, ServerFrame::Pong);
        send_frame(&tx, &slow_consumer, ServerFrame::Pong);

        tokio::time::timeout(
            std::time::Duration::from_millis(50),
            slow_consumer.notified(),
        )
        .await
        .expect("a full queue must wake the close loop");
    }
}

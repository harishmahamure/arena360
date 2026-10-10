use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Notify};
use uuid::Uuid;

use super::channel::ChannelId;
use super::frame::{ClientFrame, ServerFrame};
use super::registry::ConnectionRegistry;
use super::tenant_transport::{can_receive, check_channel, current_claims, TenantTransport};
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
    db: Arc<crate::tenancy::TenantDb>,
    registry: Arc<ConnectionRegistry>,
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
    let session_db = db.clone();
    let session_claims = claims.clone();
    let mut write_handle = tokio::spawn(async move {
        let seconds_left = session_claims
            .exp
            .unwrap_or(0)
            .saturating_sub(chrono::Utc::now().timestamp())
            .max(0) as u64;
        let expiry = tokio::time::sleep(std::time::Duration::from_secs(seconds_left));
        tokio::pin!(expiry);
        let mut recheck = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            tokio::select! {
                biased;
                _ = &mut expiry => {
                    let _ = ws_sink.send(Message::Close(Some(axum::extract::ws::CloseFrame {
                        code: 4001, reason: "Session expired".into(),
                    }))).await;
                    break;
                }
                _ = recheck.tick() => {
                    let result = current_claims(session_db.clone(), &session_claims).await;
                    if result.is_err() {
                        let code = if result.is_err() { 1013 } else { 4001 };
                        let _ = ws_sink.send(Message::Close(Some(axum::extract::ws::CloseFrame {
                            code, reason: "Session verification required".into(),
                        }))).await;
                        break;
                    }
                }
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

    // Skip revoked deliveries while paging, so they cannot starve newer allowed messages.
    // Backpressure keeps the replay inside the bounded outgoing queue.
    let transport = TenantTransport::new(db.clone());
    let mut after = 0;
    let mut sent = 0;
    'replay: loop {
        let pending = match transport.replay_after(user_id, after).await {
            Ok(rows) => rows,
            Err(error) => {
                send_frame(
                    &outgoing_tx,
                    &slow_consumer,
                    ServerFrame::error("REPLAY_UNAVAILABLE", error.to_string()),
                );
                slow_consumer.notify_one();
                break;
            }
        };
        let count = pending.len();
        for pending in pending {
            after = pending.event.id;
            match can_receive(db.clone(), &claims, &pending).await {
                Ok(true) => {}
                Ok(false) => continue,
                Err(error) => {
                    send_frame(
                        &outgoing_tx,
                        &slow_consumer,
                        ServerFrame::error("REPLAY_UNAVAILABLE", error.to_string()),
                    );
                    slow_consumer.notify_one();
                    break 'replay;
                }
            }
            let row = pending.event;
            let event = ServerFrame::Event {
                msg_id: row.id,
                channel: row.channel,
                event_type: row.event_type,
                payload: row.payload,
                ts: row.created_at,
            };
            if !matches!(
                tokio::time::timeout(std::time::Duration::from_secs(5), outgoing_tx.send(event))
                    .await,
                Ok(Ok(()))
            ) {
                slow_consumer.notify_one();
                break 'replay;
            }
            sent += 1;
            if sent >= 500 {
                break 'replay;
            }
        }
        if count < 500 {
            break;
        }
    }

    // Read loop: websocket -> process frames
    let db_clone = db.clone();
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
                        &db_clone,
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
    db: &Arc<crate::tenancy::TenantDb>,
    registry: &ConnectionRegistry,
    slow_consumer: &Notify,
) {
    let original = conn.read().await.claims.clone();
    let claims = match current_claims(db.clone(), &original).await {
        Ok(claims) => claims,
        Err(error) => {
            send_frame(
                tx,
                slow_consumer,
                ServerFrame::error("SESSION_INVALID", error.to_string()),
            );
            slow_consumer.notify_one();
            return;
        }
    };
    let transport = TenantTransport::new(db.clone());
    match frame {
        ClientFrame::Subscribe { channels } => {
            let mut subscribed = Vec::new();
            for raw in channels {
                let Some(channel) = ChannelId::parse(&raw) else {
                    send_frame(
                        tx,
                        slow_consumer,
                        ServerFrame::error("UNKNOWN_CHANNEL", format!("Unknown channel: {raw}")),
                    );
                    continue;
                };
                match check_channel(db.clone(), &claims, &channel).await {
                    Ok(()) => subscribed.push(channel.as_string()),
                    Err(error) => send_frame(
                        tx,
                        slow_consumer,
                        ServerFrame::error("FORBIDDEN_CHANNEL", error.to_string()),
                    ),
                }
            }
            let mut connection = conn.write().await;
            subscribed.retain(|ch| !connection.subscriptions.contains(ch));
            subscribed.sort();
            subscribed.dedup();
            subscribed.truncate(MAX_SUBSCRIPTIONS.saturating_sub(connection.subscriptions.len()));
            for channel in &subscribed {
                connection.subscriptions.insert(channel.clone());
            }
            let id = connection.id;
            drop(connection);
            registry.subscribe(id, &subscribed).await;
            send_frame(
                tx,
                slow_consumer,
                ServerFrame::Subscribed {
                    channels: subscribed,
                },
            );
        }
        ClientFrame::Unsubscribe { channels } => {
            let mut connection = conn.write().await;
            let channels: Vec<_> = channels
                .into_iter()
                .filter_map(|raw| ChannelId::parse(&raw).map(|ch| ch.as_string()))
                .filter(|ch| connection.subscriptions.remove(ch))
                .collect();
            let id = connection.id;
            drop(connection);
            registry.unsubscribe(id, &channels).await;
            send_frame(tx, slow_consumer, ServerFrame::Unsubscribed { channels });
        }
        ClientFrame::Ack { msg_id } => {
            if let Err(error) = transport.ack_current(msg_id, &claims).await {
                send_frame(
                    tx,
                    slow_consumer,
                    ServerFrame::error("ACK_FAILED", error.to_string()),
                );
            }
        }
        ClientFrame::Publish { channel, payload } => {
            let Some(parsed) = ChannelId::parse(&channel) else {
                send_frame(
                    tx,
                    slow_consumer,
                    ServerFrame::error("UNKNOWN_CHANNEL", "Unknown channel"),
                );
                return;
            };
            let channel = parsed.as_string();
            if !conn.read().await.subscriptions.contains(&channel) {
                send_frame(
                    tx,
                    slow_consumer,
                    ServerFrame::error("NOT_SUBSCRIBED", "Subscribe before publishing"),
                );
                return;
            }
            if let Err(error) = transport.publish_chat(&claims, channel, payload).await {
                send_frame(
                    tx,
                    slow_consumer,
                    ServerFrame::error("PUBLISH_FAILED", error.to_string()),
                );
            }
        }
        ClientFrame::Ping => send_frame(tx, slow_consumer, ServerFrame::Pong),
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

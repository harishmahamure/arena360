use chrono::{DateTime, Utc};
use prost::Message;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::proto::arena360::v1 as pb;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientFrame {
    Subscribe {
        channels: Vec<String>,
    },
    Unsubscribe {
        channels: Vec<String>,
    },
    Ack {
        msg_id: i64,
    },
    Publish {
        channel: String,
        payload: serde_json::Value,
    },
    Ping,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ServerFrame {
    Welcome {
        user_id: Uuid,
        roles: Vec<String>,
    },
    Subscribed {
        channels: Vec<String>,
    },
    Unsubscribed {
        channels: Vec<String>,
    },
    Event {
        msg_id: i64,
        channel: String,
        event_type: String,
        payload: serde_json::Value,
        ts: DateTime<Utc>,
    },
    Error {
        code: String,
        message: String,
    },
    Pong,
}

impl ServerFrame {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Error {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn encode_binary(&self) -> Vec<u8> {
        let frame = match self {
            Self::Welcome { user_id, roles } => pb::server_frame::Frame::Welcome(pb::Welcome {
                user_id: user_id.to_string(),
                roles: roles.clone(),
            }),
            Self::Subscribed { channels } => pb::server_frame::Frame::Subscribed(pb::Subscribed {
                channels: channels.clone(),
            }),
            Self::Unsubscribed { channels } => {
                pb::server_frame::Frame::Unsubscribed(pb::Unsubscribed {
                    channels: channels.clone(),
                })
            }
            Self::Event {
                msg_id,
                channel,
                event_type,
                payload,
                ts,
            } => pb::server_frame::Frame::Event(pb::Event {
                msg_id: *msg_id,
                channel: channel.clone(),
                event_type: event_type.clone(),
                payload_json: serde_json::to_vec(payload).unwrap_or_default(),
                timestamp_ms: ts.timestamp_millis(),
            }),
            Self::Error { code, message } => pb::server_frame::Frame::Error(pb::Error {
                code: code.clone(),
                message: message.clone(),
            }),
            Self::Pong => pb::server_frame::Frame::Pong(pb::Pong {}),
        };
        pb::ServerFrame { frame: Some(frame) }.encode_to_vec()
    }
}

impl ClientFrame {
    pub fn decode_binary(bytes: &[u8]) -> Result<Self, String> {
        let frame = pb::ClientFrame::decode(bytes).map_err(|error| error.to_string())?;
        Ok(
            match frame
                .frame
                .ok_or_else(|| "missing client frame".to_string())?
            {
                pb::client_frame::Frame::Subscribe(value) => Self::Subscribe {
                    channels: value.channels,
                },
                pb::client_frame::Frame::Unsubscribe(value) => Self::Unsubscribe {
                    channels: value.channels,
                },
                pb::client_frame::Frame::Ack(value) => Self::Ack {
                    msg_id: value.msg_id,
                },
                pb::client_frame::Frame::Publish(value) => Self::Publish {
                    channel: value.channel,
                    payload: serde_json::from_slice(&value.payload_json)
                        .map_err(|error| error.to_string())?,
                },
                pb::client_frame::Frame::Ping(_) => Self::Ping,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_subscribe_roundtrip() {
        let json = r#"{"type":"Subscribe","channels":["admin","staff"]}"#;
        let frame: ClientFrame = serde_json::from_str(json).unwrap();
        match frame {
            ClientFrame::Subscribe { channels } => {
                assert_eq!(channels, vec!["admin", "staff"]);
            }
            _ => panic!("expected Subscribe"),
        }
    }

    #[test]
    fn client_ack_roundtrip() {
        let json = r#"{"type":"Ack","msg_id":42}"#;
        let frame: ClientFrame = serde_json::from_str(json).unwrap();
        match frame {
            ClientFrame::Ack { msg_id } => assert_eq!(msg_id, 42),
            _ => panic!("expected Ack"),
        }
    }

    #[test]
    fn client_publish_roundtrip() {
        let json = r#"{"type":"Publish","channel":"room:support-1","payload":{"text":"hello"}}"#;
        let frame: ClientFrame = serde_json::from_str(json).unwrap();
        match frame {
            ClientFrame::Publish { channel, payload } => {
                assert_eq!(channel, "room:support-1");
                assert_eq!(payload["text"], "hello");
            }
            _ => panic!("expected Publish"),
        }
    }

    #[test]
    fn client_ping() {
        let json = r#"{"type":"Ping"}"#;
        let frame: ClientFrame = serde_json::from_str(json).unwrap();
        assert!(matches!(frame, ClientFrame::Ping));
    }

    #[test]
    fn server_welcome_serializes() {
        let frame = ServerFrame::Welcome {
            user_id: Uuid::nil(),
            roles: vec!["admin".to_string()],
        };
        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("\"type\":\"Welcome\""));
    }

    #[test]
    fn server_event_serializes() {
        let frame = ServerFrame::Event {
            msg_id: 1,
            channel: "admin".to_string(),
            event_type: "test".to_string(),
            payload: serde_json::json!({}),
            ts: Utc::now(),
        };
        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("\"type\":\"Event\""));
        assert!(json.contains("\"msg_id\":1"));
    }

    #[test]
    fn server_error_serializes() {
        let frame = ServerFrame::error("FORBIDDEN_CHANNEL", "nope");
        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("FORBIDDEN_CHANNEL"));
    }

    #[test]
    fn protobuf_client_frame_decodes() {
        let bytes = pb::ClientFrame {
            frame: Some(pb::client_frame::Frame::Subscribe(pb::Subscribe {
                channels: vec!["admin".to_string()],
            })),
        }
        .encode_to_vec();
        assert!(matches!(
            ClientFrame::decode_binary(&bytes).unwrap(),
            ClientFrame::Subscribe { .. }
        ));
    }
}

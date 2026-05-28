use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientFrame {
    Subscribe { channels: Vec<String> },
    Unsubscribe { channels: Vec<String> },
    Ack { msg_id: i64 },
    Publish { channel: String, payload: serde_json::Value },
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
}

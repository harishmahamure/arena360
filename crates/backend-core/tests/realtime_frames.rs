use gaming_cafe_api::realtime::frame::{ClientFrame, ServerFrame};

#[test]
fn deserialize_subscribe() {
    let json = r#"{"type":"Subscribe","channels":["admin","staff","user:00000000-0000-0000-0000-000000000001"]}"#;
    let frame: ClientFrame = serde_json::from_str(json).unwrap();
    match frame {
        ClientFrame::Subscribe { channels } => {
            assert_eq!(channels.len(), 3);
            assert_eq!(channels[0], "admin");
        }
        _ => panic!("Expected Subscribe"),
    }
}

#[test]
fn deserialize_unsubscribe() {
    let json = r#"{"type":"Unsubscribe","channels":["admin"]}"#;
    let frame: ClientFrame = serde_json::from_str(json).unwrap();
    assert!(matches!(frame, ClientFrame::Unsubscribe { channels } if channels == vec!["admin"]));
}

#[test]
fn deserialize_ack() {
    let json = r#"{"type":"Ack","msg_id":42}"#;
    let frame: ClientFrame = serde_json::from_str(json).unwrap();
    assert!(matches!(frame, ClientFrame::Ack { msg_id } if msg_id == 42));
}

#[test]
fn deserialize_publish() {
    let json = r#"{"type":"Publish","channel":"room:support-1","payload":{"text":"hello"}}"#;
    let frame: ClientFrame = serde_json::from_str(json).unwrap();
    match frame {
        ClientFrame::Publish { channel, payload } => {
            assert_eq!(channel, "room:support-1");
            assert_eq!(payload["text"], "hello");
        }
        _ => panic!("Expected Publish"),
    }
}

#[test]
fn deserialize_ping() {
    let json = r#"{"type":"Ping"}"#;
    let frame: ClientFrame = serde_json::from_str(json).unwrap();
    assert!(matches!(frame, ClientFrame::Ping));
}

#[test]
fn serialize_welcome() {
    let frame = ServerFrame::Welcome {
        user_id: uuid::Uuid::nil(),
        roles: vec!["admin".to_string()],
    };
    let json = serde_json::to_string(&frame).unwrap();
    assert!(json.contains("\"type\":\"Welcome\""));
    assert!(json.contains("\"roles\":[\"admin\"]"));
}

#[test]
fn serialize_event() {
    let frame = ServerFrame::Event {
        msg_id: 99,
        channel: "admin".to_string(),
        event_type: "transaction.sale_completed".to_string(),
        payload: serde_json::json!({"amount": 150.0}),
        ts: chrono::Utc::now(),
    };
    let json = serde_json::to_string(&frame).unwrap();
    assert!(json.contains("\"msg_id\":99"));
    assert!(json.contains("transaction.sale_completed"));
}

#[test]
fn serialize_error() {
    let frame = ServerFrame::error("FORBIDDEN_CHANNEL", "Only admins allowed");
    let json = serde_json::to_string(&frame).unwrap();
    assert!(json.contains("FORBIDDEN_CHANNEL"));
    assert!(json.contains("Only admins allowed"));
}

#[test]
fn serialize_pong() {
    let frame = ServerFrame::Pong;
    let json = serde_json::to_string(&frame).unwrap();
    assert!(json.contains("\"type\":\"Pong\""));
}

#[test]
fn invalid_json_fails_gracefully() {
    let result = serde_json::from_str::<ClientFrame>(r#"{"type":"Unknown"}"#);
    assert!(result.is_err());
}

#[test]
fn missing_required_field_fails() {
    let result = serde_json::from_str::<ClientFrame>(r#"{"type":"Subscribe"}"#);
    assert!(result.is_err());
}

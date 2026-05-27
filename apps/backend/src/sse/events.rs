#[derive(Clone, Debug, serde::Serialize)]
pub struct SseEvent {
    pub topic: String,
    pub payload: serde_json::Value,
}

impl SseEvent {
    pub fn new(topic: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            topic: topic.into(),
            payload,
        }
    }

    pub fn session_started(session_id: &str) -> Self {
        Self::new(
            "session.started",
            serde_json::json!({ "session_id": session_id }),
        )
    }

    pub fn session_ended(session_id: &str) -> Self {
        Self::new(
            "session.ended",
            serde_json::json!({ "session_id": session_id }),
        )
    }

    pub fn device_status_changed(device_id: &str, status: &str) -> Self {
        Self::new(
            "device.status_changed",
            serde_json::json!({ "device_id": device_id, "status": status }),
        )
    }

    pub fn plan_exhausted(player_plan_id: &str) -> Self {
        Self::new(
            "plan.exhausted",
            serde_json::json!({ "player_plan_id": player_plan_id }),
        )
    }

    pub fn transaction_created(transaction_id: &str) -> Self {
        Self::new(
            "transaction.created",
            serde_json::json!({ "transaction_id": transaction_id }),
        )
    }
}

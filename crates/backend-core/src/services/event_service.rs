use crate::sse::Broadcaster;

/// Legacy event service — now a no-op shim.
/// Real-time events flow through `realtime::OutboxService` (ADR-0013).
/// Retained so existing service constructors compile without churn.
#[derive(Clone)]
pub struct EventService {
    _broadcaster: Broadcaster,
}

impl EventService {
    pub fn new(broadcaster: Broadcaster) -> Self {
        Self {
            _broadcaster: broadcaster,
        }
    }

    pub fn publish_device_status(&self, _device_id: &str, _status: &str) {}

    pub fn publish_session_started(&self, _session_id: &str) {}

    pub fn publish_session_ended(&self, _session_id: &str) {}

    pub fn publish_transaction_created(&self, _transaction_id: &str) {}
}

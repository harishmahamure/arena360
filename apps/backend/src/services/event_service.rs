use crate::sse::{Broadcaster, SseEvent};

#[derive(Clone)]
pub struct EventService {
    broadcaster: Broadcaster,
}

impl EventService {
    pub fn new(broadcaster: Broadcaster) -> Self {
        Self { broadcaster }
    }

    pub fn publish_device_status(&self, device_id: &str, status: &str) {
        self.broadcaster
            .publish(SseEvent::device_status_changed(device_id, status));
    }

    pub fn publish_session_started(&self, session_id: &str) {
        self.broadcaster
            .publish(SseEvent::session_started(session_id));
    }

    pub fn publish_session_ended(&self, session_id: &str) {
        self.broadcaster
            .publish(SseEvent::session_ended(session_id));
    }

    pub fn publish_transaction_created(&self, transaction_id: &str) {
        self.broadcaster
            .publish(SseEvent::transaction_created(transaction_id));
    }

    pub fn broadcaster(&self) -> Broadcaster {
        self.broadcaster.clone()
    }
}

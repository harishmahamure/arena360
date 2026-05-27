use tokio::sync::broadcast;

use super::events::SseEvent;

#[derive(Clone)]
pub struct Broadcaster {
    sender: broadcast::Sender<SseEvent>,
}

impl Broadcaster {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn publish(&self, event: SseEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SseEvent> {
        self.sender.subscribe()
    }
}

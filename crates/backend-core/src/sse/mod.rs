/// Legacy SSE module — replaced by `realtime::` WebSocket channel (ADR-0013).
/// This file provides a no-op `Broadcaster` so existing `EventService` compiles.
/// `EventService` calls are dead code that will be removed in a follow-up.

#[derive(Clone)]
pub struct Broadcaster;

impl Broadcaster {
    pub fn new(_capacity: usize) -> Self {
        Self
    }
}

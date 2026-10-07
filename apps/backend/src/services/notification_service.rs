//! Notification payloads consumed by the atomic tenant activity writer.
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum Recipients {
    AllAdmins,
    AllStaff,
    Users(Vec<Uuid>),
    /// Admins plus specific users (e.g. approval requester).
    AdminAndUsers(Vec<Uuid>),
}

#[derive(Debug, Clone)]
pub struct RecordNotification {
    pub kind: String,
    pub title: String,
    pub summary: Option<String>,
    pub payload: Value,
    pub actor_user_id: Option<Uuid>,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub recipients: Recipients,
}

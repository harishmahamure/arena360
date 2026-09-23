//! Integration tests for notification and activity log APIs.
//! Run with: `cargo test --test notifications -- --ignored`

use gaming_cafe_api::app::build_state;
use gaming_cafe_api::config::load_dotenv;
use gaming_cafe_api::models::NotificationFilterDto;
use gaming_cafe_api::services::{RecordNotification, Recipients};
use std::sync::Arc;
use uuid::Uuid;

async fn setup() -> Option<Arc<gaming_cafe_api::app::AppState>> {
    load_dotenv();
    if std::env::var("DATABASE_URL").is_err() && std::env::var("DB_HOST").is_err() {
        return None;
    }
    Some(build_state().await)
}

async fn active_staff_user_id(state: &gaming_cafe_api::app::AppState) -> Option<Uuid> {
    sqlx::query_scalar(
        r#"SELECT id FROM users
           WHERE role = 'staff' AND "isActive" = true AND "deletedAt" IS NULL
           LIMIT 1"#,
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn record_and_list_notifications_for_user() {
    let Some(state) = setup().await else {
        return;
    };

    let Some(user_id) = active_staff_user_id(&state).await else {
        return;
    };

    let activity = state
        .notifications
        .record(RecordNotification {
            kind: "kiosk_order_placed".to_string(),
            title: "Test kiosk order".to_string(),
            summary: Some("Integration test".to_string()),
            payload: serde_json::json!({ "test": true }),
            actor_user_id: None,
            entity_type: Some("kiosk_order".to_string()),
            entity_id: Some(Uuid::new_v4()),
            recipients: Recipients::AllStaff,
        })
        .await
        .expect("record notification");

    assert_eq!(activity.title, "Test kiosk order");

    let inbox = state
        .notifications
        .list_notifications(
            user_id,
            NotificationFilterDto {
                page: Some(1),
                limit: Some(10),
                unread_only: Some(false),
                important_only: None,
            },
        )
        .await
        .expect("list notifications");

    assert!(!inbox.data.is_empty());
    assert!(inbox.data.iter().any(|n| n.activity_id == activity.id));

    let unread = state
        .notifications
        .unread_count(user_id, NotificationFilterDto::default())
        .await
        .expect("unread count");
    assert!(unread.count >= 1);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn non_order_activities_do_not_create_notifications() {
    let Some(state) = setup().await else {
        return;
    };

    let Some(user_id) = active_staff_user_id(&state).await else {
        return;
    };

    let session_activity = state
        .notifications
        .record(RecordNotification {
            kind: "session_started".to_string(),
            title: "Session started".to_string(),
            summary: None,
            payload: serde_json::json!({}),
            actor_user_id: None,
            entity_type: Some("session".to_string()),
            entity_id: Some(Uuid::new_v4()),
            recipients: Recipients::Users(vec![user_id]),
        })
        .await
        .expect("record session activity");

    let approval_activity = state
        .notifications
        .record(RecordNotification {
            kind: "approval_requested".to_string(),
            title: "Approval needed".to_string(),
            summary: None,
            payload: serde_json::json!({}),
            actor_user_id: None,
            entity_type: Some("expense".to_string()),
            entity_id: Some(Uuid::new_v4()),
            recipients: Recipients::Users(vec![user_id]),
        })
        .await
        .expect("record approval activity");

    let order_activity = state
        .notifications
        .record(RecordNotification {
            kind: "kiosk_order_placed".to_string(),
            title: "Kiosk order".to_string(),
            summary: None,
            payload: serde_json::json!({}),
            actor_user_id: None,
            entity_type: Some("kiosk_order".to_string()),
            entity_id: Some(Uuid::new_v4()),
            recipients: Recipients::AllStaff,
        })
        .await
        .expect("record kiosk order");

    let inbox = state
        .notifications
        .list_notifications(
            user_id,
            NotificationFilterDto {
                limit: Some(10),
                ..Default::default()
            },
        )
        .await
        .expect("list notifications");

    assert!(inbox.data.iter().any(|n| n.activity_id == order_activity.id));
    assert!(!inbox.data.iter().any(|n| n.activity_id == session_activity.id));
    assert!(!inbox.data.iter().any(|n| n.activity_id == approval_activity.id));
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn mark_notification_read_removes_order_from_unread_inbox() {
    let Some(state) = setup().await else {
        return;
    };

    let Some(user_id) = active_staff_user_id(&state).await else {
        return;
    };

    let activity = state
        .notifications
        .record(RecordNotification {
            kind: "kiosk_order_placed".to_string(),
            title: "Kiosk order".to_string(),
            summary: None,
            payload: serde_json::json!({}),
            actor_user_id: None,
            entity_type: Some("kiosk_order".to_string()),
            entity_id: Some(Uuid::new_v4()),
            recipients: Recipients::AllStaff,
        })
        .await
        .expect("record");

    let inbox = state
        .notifications
        .list_notifications(
            user_id,
            NotificationFilterDto {
                unread_only: Some(true),
                ..Default::default()
            },
        )
        .await
        .expect("list");

    let notification_id = inbox
        .data
        .iter()
        .find(|notification| notification.activity_id == activity.id)
        .expect("new order notification")
        .id;
    let updated = state
        .notifications
        .mark_read(notification_id, user_id)
        .await
        .expect("mark read");
    assert!(updated);

    let unread = state
        .notifications
        .list_notifications(
            user_id,
            NotificationFilterDto {
                unread_only: Some(true),
                ..Default::default()
            },
        )
        .await
        .expect("list unread");
    assert!(!unread.data.iter().any(|n| n.activity_id == activity.id));
}

#[test]
fn notification_service_recipients_users_variant() {
    let id = Uuid::new_v4();
    match Recipients::Users(vec![id]) {
        Recipients::Users(ids) => assert_eq!(ids, vec![id]),
        _ => panic!("expected Users"),
    }
}

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

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn record_and_list_notifications_for_user() {
    let Some(state) = setup().await else {
        return;
    };

    let user_id = Uuid::new_v4();

    let activity = state
        .notifications
        .record(RecordNotification {
            kind: "credit_settlement".to_string(),
            title: "Test settlement".to_string(),
            summary: Some("Integration test".to_string()),
            payload: serde_json::json!({ "test": true }),
            actor_user_id: Some(user_id),
            entity_type: Some("credit_settlement".to_string()),
            entity_id: Some(Uuid::new_v4()),
            recipients: Recipients::Users(vec![user_id]),
        })
        .await
        .expect("record notification");

    assert_eq!(activity.title, "Test settlement");

    let inbox = state
        .notifications
        .list_notifications(
            user_id,
            NotificationFilterDto {
                page: Some(1),
                limit: Some(10),
                unread_only: Some(false),
            },
        )
        .await
        .expect("list notifications");

    assert!(!inbox.data.is_empty());
    assert!(inbox.data.iter().any(|n| n.activity_id == activity.id));

    let unread = state
        .notifications
        .unread_count(user_id)
        .await
        .expect("unread count");
    assert!(unread.count >= 1);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn mark_notification_read_clears_unread_count() {
    let Some(state) = setup().await else {
        return;
    };

    let user_id = Uuid::new_v4();

    state
        .notifications
        .record(RecordNotification {
            kind: "shift_clock_in".to_string(),
            title: "Shift started".to_string(),
            summary: None,
            payload: serde_json::json!({}),
            actor_user_id: Some(user_id),
            entity_type: None,
            entity_id: None,
            recipients: Recipients::Users(vec![user_id]),
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

    let notification_id = inbox.data[0].id;
    let updated = state
        .notifications
        .mark_read(notification_id, user_id)
        .await
        .expect("mark read");
    assert!(updated);

    let unread = state
        .notifications
        .unread_count(user_id)
        .await
        .expect("unread");
    assert_eq!(unread.count, 0);
}

#[test]
fn notification_service_recipients_users_variant() {
    let id = Uuid::new_v4();
    match Recipients::Users(vec![id]) {
        Recipients::Users(ids) => assert_eq!(ids, vec![id]),
        _ => panic!("expected Users"),
    }
}

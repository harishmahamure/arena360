//! Tenant-local activity and venue-scoped notification inbox APIs.
mod support;
use gaming_cafe_api::{
    models::NotificationFilterDto,
    repositories::TenantNotificationRepository,
    services::{Recipients, RecordNotification},
};
use serde_json::json;
use support::TenantFixture;
use uuid::Uuid;
fn input(kind: &str, recipients: Recipients) -> RecordNotification {
    RecordNotification {
        kind: kind.into(),
        title: "Test activity".into(),
        summary: None,
        payload: json!({"test":true}),
        actor_user_id: None,
        entity_type: Some("fixture".into()),
        entity_id: Some(Uuid::now_v7()),
        recipients,
    }
}
async fn setup() -> (TenantFixture, TenantNotificationRepository, Uuid, Uuid) {
    let f = TenantFixture::new().await;
    let venue = f.venue("main").await;
    let user = f
        .staff(Some(venue), vec!["notifications:read".into()])
        .await;
    let repo = TenantNotificationRepository::new(f.db.clone());
    (f, repo, user, venue)
}
#[tokio::test]
async fn record_and_list_notifications_for_user() {
    let (f, repo, user, venue) = setup().await;
    let activity = repo
        .record_at(input("kiosk_order_placed", Recipients::AllStaff), venue)
        .await
        .unwrap();
    let inbox = repo
        .list_notifications(user, &NotificationFilterDto::default())
        .await
        .unwrap();
    assert_eq!(inbox.total, 1);
    assert_eq!(inbox.data[0].activity_id, activity.id);
    assert_eq!(repo.unread_count(user, false).await.unwrap(), 1);
    let foreign = f.staff(None, vec!["notifications:read".into()]).await;
    assert_eq!(repo.unread_count(foreign, false).await.unwrap(), 0);
    f.close().await;
}
#[tokio::test]
async fn non_order_activities_do_not_create_notifications() {
    let (f, repo, user, venue) = setup().await;
    let session = repo
        .record(input("session_started", Recipients::Users(vec![user])))
        .await
        .unwrap();
    let approval = repo
        .record(input("approval_requested", Recipients::Users(vec![user])))
        .await
        .unwrap();
    let order = repo
        .record_at(input("kiosk_order_placed", Recipients::AllStaff), venue)
        .await
        .unwrap();
    let inbox = repo
        .list_notifications(user, &NotificationFilterDto::default())
        .await
        .unwrap();
    assert_eq!(inbox.total, 1);
    assert_eq!(inbox.data[0].activity_id, order.id);
    assert!(!inbox
        .data
        .iter()
        .any(|n| [session.id, approval.id].contains(&n.activity_id)));
    f.close().await;
}
#[tokio::test]
async fn mark_notification_read_removes_order_from_unread_inbox() {
    let (f, repo, user, venue) = setup().await;
    repo.record_at(input("kiosk_order_placed", Recipients::AllStaff), venue)
        .await
        .unwrap();
    let filter = NotificationFilterDto {
        unread_only: Some(true),
        ..Default::default()
    };
    let inbox = repo.list_notifications(user, &filter).await.unwrap();
    let id = inbox.data[0].id;
    let foreign = f.staff(None, vec![]).await;
    assert!(!repo.mark_read(id, foreign).await.unwrap());
    assert!(repo.mark_read(id, user).await.unwrap());
    assert_eq!(
        repo.list_notifications(user, &filter).await.unwrap().total,
        0
    );
    assert_eq!(repo.unread_count(user, false).await.unwrap(), 0);
    f.close().await;
}

mod support;
use futures::future::BoxFuture;
use gaming_cafe_api::{
    analytics::publisher::{backlog, publish_batch, EventSink, TenantEvent},
    error::AppError,
    metrics::Metrics,
    tenancy::{write_outbox_event_on_connection, NewOutboxEvent},
};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use support::TenantFixture;
use uuid::Uuid;

#[derive(Default)]
struct Sink {
    events: Mutex<Vec<Value>>,
    fail_at: AtomicUsize,
}
impl EventSink for Sink {
    fn publish<'a>(&'a self, event: &'a TenantEvent) -> BoxFuture<'a, Result<(), AppError>> {
        Box::pin(async move {
            if event.sequence as usize == self.fail_at.load(Ordering::Relaxed) {
                return Err(AppError::Internal("lost ack".into()));
            }
            self.events
                .lock()
                .unwrap()
                .push(serde_json::to_value(event).unwrap());
            Ok(())
        })
    }
}
async fn add(f: &TenantFixture, n: usize) {
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            for _ in 0..n {
                write_outbox_event_on_connection(
                    c,
                    NewOutboxEvent {
                        location_id: Some(Uuid::now_v7()),
                        aggregate_type: "transaction".into(),
                        aggregate_id: Uuid::now_v7(),
                        event_type: "transaction.updated".into(),
                        schema_version: 1,
                        deleted: false,
                        payload: serde_json::from_str(r#"{"amount":922337203685477.5807}"#)
                            .unwrap(),
                    },
                )
                .await?;
            }
            Ok(())
        })
    })
    .await
    .unwrap();
}
async fn project(f: &TenantFixture, sequence: i64) {
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE realtime_projection_cursor SET sequence=? WHERE singleton=1")
                .bind(sequence)
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
}
async fn ack(f: &TenantFixture) -> i64 {
    sqlx::query_scalar("SELECT acknowledged_sequence FROM outbox_publish_state")
        .fetch_one(&f.db.read_pool().unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn durable_ack_waits_for_realtime_and_survives_reopen_without_republishing() {
    let f = TenantFixture::new().await;
    add(&f, 2).await;
    let sink = Sink::default();
    let metrics = Metrics::default();
    publish_batch(f.db.clone(), &sink, &metrics).await.unwrap();
    assert_eq!(ack(&f).await, 2);
    assert_eq!(backlog(&f.db).await.unwrap().pending, 2);
    let events = sink.events.lock().unwrap().clone();
    assert_eq!(events[0]["tenant_id"], json!(f.db.tenant_id()));
    assert_eq!(
        events
            .iter()
            .map(|e| e["sequence"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        events[0]["payload"]["amount"].to_string(),
        "922337203685477.5807"
    );
    assert!(events[0]["occurred_at"].as_str().unwrap().ends_with('Z'));
    f.db.close().await.unwrap();
    let reopened = f.manager.open(f.db.tenant_id()).await.unwrap();
    publish_batch(reopened.clone(), &sink, &metrics)
        .await
        .unwrap();
    assert_eq!(sink.events.lock().unwrap().len(), 2);
    reopened
        .with_immediate_writer(|c| {
            Box::pin(async move {
                sqlx::query("UPDATE realtime_projection_cursor SET sequence=2")
                    .execute(c)
                    .await?;
                Ok(())
            })
        })
        .await
        .unwrap();
    publish_batch(reopened.clone(), &sink, &metrics)
        .await
        .unwrap();
    assert_eq!(backlog(&reopened).await.unwrap().pending, 0);
    // AUTOINCREMENT survives deletion and the next canonical event remains ordered.
    reopened
        .with_immediate_writer(|c| {
            Box::pin(async move {
                write_outbox_event_on_connection(
                    c,
                    NewOutboxEvent {
                        location_id: None,
                        aggregate_type: "session".into(),
                        aggregate_id: Uuid::now_v7(),
                        event_type: "session.deleted".into(),
                        schema_version: 1,
                        deleted: true,
                        payload: json!({}),
                    },
                )
                .await?;
                Ok(())
            })
        })
        .await
        .unwrap();
    publish_batch(reopened.clone(), &sink, &metrics)
        .await
        .unwrap();
    assert_eq!(sink.events.lock().unwrap()[2]["sequence"], 3);
    assert_eq!(sink.events.lock().unwrap()[2]["deleted"], true);
    reopened.close().await.unwrap();
    f.close().await;
}

#[tokio::test]
async fn outage_retains_rows_partial_ack_checkpoints_and_recovery_drains_in_order() {
    let f = TenantFixture::new().await;
    add(&f, 3).await;
    project(&f, 3).await;
    let sink = Sink::default();
    let metrics = Metrics::default();
    sink.fail_at.store(1, Ordering::Relaxed);
    assert!(publish_batch(f.db.clone(), &sink, &metrics).await.is_err());
    assert_eq!(ack(&f).await, 0);
    assert_eq!(backlog(&f.db).await.unwrap().pending, 3);
    sink.fail_at.store(2, Ordering::Relaxed);
    assert!(publish_batch(f.db.clone(), &sink, &metrics).await.is_err());
    assert_eq!(ack(&f).await, 1);
    assert_eq!(backlog(&f.db).await.unwrap().pending, 2);
    sink.fail_at.store(0, Ordering::Relaxed);
    publish_batch(f.db.clone(), &sink, &metrics).await.unwrap();
    assert_eq!(ack(&f).await, 3);
    assert_eq!(backlog(&f.db).await.unwrap().pending, 0);
    assert_eq!(
        sink.events
            .lock()
            .unwrap()
            .iter()
            .map(|e| e["sequence"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    f.close().await;
}

#[tokio::test]
async fn fencing_and_gaps_fail_closed_and_empty_polls_allow_idle_eviction() {
    let f = TenantFixture::new_with_idle(Duration::from_millis(50)).await;
    let sink = Sink::default();
    let metrics = Metrics::default();
    for _ in 0..3 {
        publish_batch(f.db.clone(), &sink, &metrics).await.unwrap();
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(f.manager.reap_idle().await.unwrap(), 1);
    let reopened = f.manager.open(f.db.tenant_id()).await.unwrap();
    reopened.close().await.unwrap();
    f.close().await;
    let f = TenantFixture::new().await;
    add(&f, 2).await;
    f.db.with_immediate_writer(|c| {
        Box::pin(async move {
            sqlx::query("DELETE FROM outbox_events WHERE sequence=1")
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(publish_batch(f.db.clone(), &sink, &metrics).await.is_err());
    assert!(sink.events.lock().unwrap().is_empty());
    assert_eq!(ack(&f).await, 0);
    f.revoke();
    assert!(publish_batch(f.db.clone(), &sink, &metrics).await.is_err());
    assert!(backlog(&f.db).await.is_err());
    f.close().await;
}

struct RevokeAfterAck<'a>(&'a TenantFixture);
impl EventSink for RevokeAfterAck<'_> {
    fn publish<'a>(&'a self, _: &'a TenantEvent) -> BoxFuture<'a, Result<(), AppError>> {
        Box::pin(async move {
            self.0.revoke();
            Ok(())
        })
    }
}
#[tokio::test]
async fn ownership_loss_after_network_ack_cannot_checkpoint_or_delete() {
    let f = TenantFixture::new().await;
    add(&f, 1).await;
    project(&f, 1).await;
    assert!(
        publish_batch(f.db.clone(), &RevokeAfterAck(&f), &Metrics::default())
            .await
            .is_err()
    );
    assert_eq!(ack(&f).await, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox_events")
            .fetch_one(&f.db.read_pool().unwrap())
            .await
            .unwrap(),
        1
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requires NATS_TEST_URL pointing to a disposable JetStream server"]
async fn jetstream_ack_deduplicates_lost_checkpoints_and_rejects_missing_stream() {
    use async_nats::jetstream::stream::{Config, RetentionPolicy, StorageType};
    use gaming_cafe_api::analytics::publisher::{JetStreamSink, TENANT_EVENT_STREAM};
    let url = std::env::var("NATS_TEST_URL").expect("explicit disposable NATS test URL");
    let client = async_nats::connect(&url).await.unwrap();
    let js = async_nats::jetstream::new(client);
    assert!(
        js.get_stream(TENANT_EVENT_STREAM).await.is_err(),
        "test requires an empty disposable NATS server"
    );
    let f = TenantFixture::new().await;
    add(&f, 2).await;
    project(&f, 2).await;
    let sink = JetStreamSink::connect(&url).await.unwrap();
    let metrics = Metrics::default();
    assert!(publish_batch(f.db.clone(), &sink, &metrics).await.is_err());
    assert_eq!(ack(&f).await, 0);
    assert_eq!(backlog(&f.db).await.unwrap().pending, 2);
    let mut stream = js
        .create_stream(Config {
            name: TENANT_EVENT_STREAM.into(),
            subjects: vec!["arena.tenant.*.events.v1".into()],
            retention: RetentionPolicy::Limits,
            storage: StorageType::File,
            max_age: Duration::from_secs(7 * 24 * 60 * 60),
            duplicate_window: Duration::from_secs(120),
            ..Default::default()
        })
        .await
        .unwrap();
    // Simulate a crash after server ACK but before the SQLite checkpoint.
    let pool = f.db.read_pool().unwrap();
    let mut event:TenantEvent=sqlx::query_as("SELECT sequence,event_id,location_id,aggregate_type,aggregate_id,event_type,occurred_at,schema_version,deleted,payload,analytics_snapshot FROM outbox_events WHERE sequence=1").fetch_one(&pool).await.unwrap();
    event.tenant_id = f.db.tenant_id();
    sink.publish(&event).await.unwrap();
    publish_batch(f.db.clone(), &sink, &metrics).await.unwrap();
    assert_eq!(ack(&f).await, 2);
    assert_eq!(backlog(&f.db).await.unwrap().pending, 0);
    assert_eq!(stream.info().await.unwrap().state.messages, 2);
    let message: async_nats::jetstream::message::StreamMessage =
        stream.get_raw_message(1).await.unwrap().try_into().unwrap();
    let envelope: Value = serde_json::from_slice(&message.payload).unwrap();
    assert_eq!(envelope["tenant_id"], json!(f.db.tenant_id()));
    assert_eq!(envelope["event_id"], event.event_id);
    assert_eq!(
        message.subject.as_str(),
        format!("arena.tenant.{}.events.v1", f.db.tenant_id())
    );
    f.close().await;
}

struct WriterDuringPublish(Arc<gaming_cafe_api::tenancy::TenantDb>);
impl EventSink for WriterDuringPublish {
    fn publish<'a>(&'a self, _: &'a TenantEvent) -> BoxFuture<'a, Result<(), AppError>> {
        Box::pin(async move {
            tokio::time::timeout(
                Duration::from_secs(1),
                self.0.with_immediate_writer(|_| Box::pin(async { Ok(()) })),
            )
            .await
            .map_err(|_| AppError::Internal("network publish held SQLite writer".into()))??;
            Ok(())
        })
    }
}
#[tokio::test]
async fn publishing_does_not_hold_the_operational_writer() {
    let f = TenantFixture::new().await;
    add(&f, 1).await;
    publish_batch(
        f.db.clone(),
        &WriterDuringPublish(f.db.clone()),
        &Metrics::default(),
    )
    .await
    .unwrap();
    assert_eq!(ack(&f).await, 1);
    f.close().await;
}

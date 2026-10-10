use async_nats::jetstream::stream::{Config, RetentionPolicy};
use gaming_cafe_api::analytics::publisher::TENANT_EVENT_STREAM;
use std::{process::Command, time::Duration};
fn setup(url: &str, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tenant_events_setup"))
        .args(args)
        .env("NATS_URL", url)
        .env("NATS_STREAM_REPLICAS", "1")
        .output()
        .unwrap()
}
#[tokio::test]
#[ignore = "requires NATS_TEST_URL pointing to an empty disposable JetStream server"]
async fn setup_preserves_replay_and_existing_messages_and_refuses_incompatible_streams() {
    let url = std::env::var("NATS_TEST_URL").expect("explicit disposable NATS URL");
    let js = async_nats::jetstream::new(async_nats::connect(&url).await.unwrap());
    assert!(
        js.get_stream(TENANT_EVENT_STREAM).await.is_err(),
        "test needs an empty disposable server"
    );
    assert!(!setup(&url, &["--check"]).status.success());
    assert!(js.get_stream(TENANT_EVENT_STREAM).await.is_err());
    let output = setup(&url, &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut stream = js.get_stream(TENANT_EVENT_STREAM).await.unwrap();
    let info = stream.info().await.unwrap();
    assert_eq!(info.config.retention, RetentionPolicy::Limits);
    assert_eq!(info.config.max_age, Duration::from_secs(7 * 24 * 60 * 60));
    assert_eq!(info.config.subjects, vec!["arena.tenant.*.events.v1"]);
    js.publish("arena.tenant.test.events.v1", "replay fixture".into())
        .await
        .unwrap()
        .await
        .unwrap();
    assert!(setup(&url, &[]).status.success());
    assert!(setup(&url, &["--check"]).status.success());
    assert_eq!(stream.info().await.unwrap().state.messages, 1);
    // Consumer ACK cannot remove replay history under limits retention.
    let consumer = stream
        .create_consumer(async_nats::jetstream::consumer::pull::Config {
            durable_name: Some("stream_setup_check".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    use futures::StreamExt;
    let mut messages = consumer.fetch().max_messages(1).messages().await.unwrap();
    let message = messages.next().await.unwrap().unwrap();
    message.ack().await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(stream.info().await.unwrap().state.messages, 1);
    // Only this disposable test fixture is replaced; the CLI never updates or purges.
    js.delete_stream(TENANT_EVENT_STREAM).await.unwrap();
    let mut incompatible = js
        .create_stream(Config {
            name: TENANT_EVENT_STREAM.into(),
            subjects: vec!["arena.tenant.*.events.v1".into()],
            retention: RetentionPolicy::WorkQueue,
            ..Default::default()
        })
        .await
        .unwrap();
    js.publish("arena.tenant.test.events.v1", "preserve fixture".into())
        .await
        .unwrap()
        .await
        .unwrap();
    assert!(!setup(&url, &[]).status.success());
    let info = incompatible.info().await.unwrap();
    assert_eq!(info.config.retention, RetentionPolicy::WorkQueue);
    assert_eq!(info.state.messages, 1);
}

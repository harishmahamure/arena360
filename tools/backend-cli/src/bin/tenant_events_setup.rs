//! Explicit, idempotent provisioning; incompatible existing streams are never changed.
use async_nats::jetstream::stream::{Config, RetentionPolicy, StorageType};
use gaming_cafe_api::analytics::publisher::TENANT_EVENT_STREAM;
use std::time::Duration;
const SUBJECT: &str = "arena.tenant.*.events.v1";
const RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);
fn config(replicas: usize) -> Config {
    Config {
        name: TENANT_EVENT_STREAM.into(),
        subjects: vec![SUBJECT.into()],
        retention: RetentionPolicy::Limits,
        storage: StorageType::File,
        max_age: RETENTION,
        num_replicas: replicas,
        duplicate_window: Duration::from_secs(120),
        deny_delete: true,
        deny_purge: true,
        ..Default::default()
    }
}
fn validate(c: &Config, replicas: usize) -> Result<(), &'static str> {
    if c.name != TENANT_EVENT_STREAM
        || c.subjects != [SUBJECT]
        || c.retention != RetentionPolicy::Limits
        || c.storage != StorageType::File
        || c.max_age != RETENTION
        || c.num_replicas != replicas
        || c.no_ack
        || c.duplicate_window < Duration::from_secs(120)
        || c.sealed
        || c.allow_rollup
        || c.max_messages > 0
        || c.max_bytes > 0
        || c.max_messages_per_subject > 0
        || c.mirror.is_some()
        || c.sources.as_ref().is_some_and(|s| !s.is_empty())
    {
        return Err("Existing tenant event stream is incompatible; reconcile it explicitly before enabling publication (no configuration or messages were changed)");
    }
    Ok(())
}
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Tenant event stream setup failed: {error}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("tenant_events_setup [--check]\nRequires NATS_URL; NATS_STREAM_REPLICAS=1 (or 3/5 on a cluster). Creates/verifies ARENA_TENANT_EVENTS with file storage and seven-day limits retention. --check never creates a stream.");
        return Ok(());
    }
    if !args.is_empty() && args != ["--check"] {
        return Err("Use --help or --check".into());
    }
    gaming_cafe_api::config::load_dotenv();
    let url = std::env::var("NATS_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .ok_or("Set NATS_URL explicitly")?;
    let replicas: usize = std::env::var("NATS_STREAM_REPLICAS")
        .unwrap_or_else(|_| "1".into())
        .parse()
        .map_err(|_| "NATS_STREAM_REPLICAS must be 1, 3 or 5")?;
    if ![1, 3, 5].contains(&replicas) {
        return Err("NATS_STREAM_REPLICAS must be 1, 3 or 5".into());
    }
    let client = tokio::time::timeout(Duration::from_secs(5), async_nats::connect(url))
        .await
        .map_err(|_| "NATS connection timed out")?
        .map_err(|_| "NATS connection failed")?;
    let mut js = async_nats::jetstream::new(client);
    js.set_timeout(Duration::from_secs(5));
    let mut stream = if args == ["--check"] {
        js.get_stream(TENANT_EVENT_STREAM)
            .await
            .map_err(|_| "Cannot read tenant event stream")?
    } else {
        js.get_or_create_stream(config(replicas))
            .await
            .map_err(|_| "Cannot create or read tenant event stream")?
    };
    let info = stream
        .info()
        .await
        .map_err(|_| "Cannot verify tenant event stream")?;
    validate(&info.config, replicas)?;
    println!("ARENA_TENANT_EVENTS ready: subject={SUBJECT}; file storage; limits retention=7 days; replicas={replicas}");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_replay_configuration_and_normalized_unlimited_caps() {
        let mut c = config(1);
        assert!(validate(&c, 1).is_ok());
        c.max_messages = -1;
        c.max_bytes = -1;
        c.max_messages_per_subject = -1;
        assert!(validate(&c, 1).is_ok());
    }
    #[test]
    fn rejects_work_queue_expiry_loss_memory_and_unacknowledged_streams() {
        for mutation in 0..7 {
            let mut c = config(1);
            match mutation {
                0 => c.retention = RetentionPolicy::WorkQueue,
                1 => c.max_age = Duration::from_secs(3600),
                2 => c.storage = StorageType::Memory,
                3 => c.no_ack = true,
                4 => c.max_messages = 100,
                5 => c.subjects = vec!["arena.>".into()],
                _ => c.sealed = true,
            };
            assert!(validate(&c, 1).is_err());
        }
    }
    #[test]
    fn rejects_replica_and_duplicate_window_mismatches() {
        assert!(validate(&config(1), 3).is_err());
        let mut c = config(1);
        c.duplicate_window = Duration::ZERO;
        assert!(validate(&c, 1).is_err());
    }
}

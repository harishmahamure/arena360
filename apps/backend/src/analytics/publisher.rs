//! Ordered, lease-fenced publication of canonical tenant snapshots.
use crate::{
    error::AppError,
    metrics::Metrics,
    tenancy::{TenantDb, TenantDbManager},
};
use futures::future::BoxFuture;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use sqlx::FromRow;
use std::{collections::HashSet, sync::Arc, time::Duration};
use uuid::Uuid;

pub const TENANT_EVENT_STREAM: &str = "ARENA_TENANT_EVENTS";
const BATCH_SIZE: i64 = 250;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TenantEvent {
    pub sequence: i64,
    pub event_id: String,
    #[sqlx(skip)]
    pub tenant_id: Uuid,
    pub location_id: Option<String>,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub occurred_at: String,
    pub schema_version: i64,
    pub deleted: bool,
    #[sqlx(json)]
    pub payload: Value,
    #[serde(default)]
    #[sqlx(json(nullable))]
    pub analytics_snapshot: Option<crate::tenancy::analytics_snapshot::AnalyticsSnapshot>,
}

/// Returning Ok means the named stream durably acknowledged the message.
pub trait EventSink: Send + Sync {
    fn publish<'a>(&'a self, event: &'a TenantEvent) -> BoxFuture<'a, Result<(), AppError>>;
}

pub struct JetStreamSink(async_nats::jetstream::Context);
impl JetStreamSink {
    pub async fn connect(url: &str) -> Result<Self, AppError> {
        let client = tokio::time::timeout(Duration::from_secs(1), async_nats::connect(url))
            .await
            .map_err(|_| AppError::Internal("NATS connection timed out".into()))?
            .map_err(|_| AppError::Internal("NATS connection failed".into()))?;
        let mut context = async_nats::jetstream::new(client);
        context.set_timeout(Duration::from_secs(1));
        Ok(Self(context))
    }
}
impl EventSink for JetStreamSink {
    fn publish<'a>(&'a self, event: &'a TenantEvent) -> BoxFuture<'a, Result<(), AppError>> {
        Box::pin(async move {
            let mut headers = async_nats::HeaderMap::new();
            headers.insert(
                "Nats-Msg-Id",
                format!("{}:{}", event.tenant_id, event.event_id),
            );
            // The server rejects an accidentally configured overlapping stream.
            headers.insert("Nats-Expected-Stream", TENANT_EVENT_STREAM);
            let body = serde_json::to_vec(event).map_err(|e| AppError::Internal(e.to_string()))?;
            let ack = self
                .0
                .publish_with_headers(
                    format!("arena.tenant.{}.events.v1", event.tenant_id),
                    headers,
                    body.into(),
                )
                .await
                .map_err(|_| AppError::Internal("JetStream publish failed".into()))?
                .await
                .map_err(|_| AppError::Internal("JetStream acknowledgement failed".into()))?;
            if ack.stream != TENANT_EVENT_STREAM || ack.sequence == 0 {
                return Err(AppError::Internal(
                    "Unexpected JetStream acknowledgement".into(),
                ));
            }
            Ok(())
        })
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Backlog {
    pub pending: u64,
    pub bytes: u64,
    pub oldest_millis: u64,
}

pub async fn backlog(db: &TenantDb) -> Result<Backlog, AppError> {
    db.ensure_current_owner()?;
    let row: (i64, i64, Option<String>) = sqlx::query_as(
        "SELECT COUNT(*),COALESCE(SUM(length(CAST(payload AS BLOB))+COALESCE(length(CAST(analytics_snapshot AS BLOB)),0)+length(event_id)+length(aggregate_id)+length(aggregate_type)+length(event_type)+length(occurred_at)+COALESCE(length(location_id),0)+25),0),MIN(occurred_at) FROM outbox_events"
    ).fetch_one(&db.background_read_pool()?).await?;
    let oldest_millis = row
        .2
        .map(|s| {
            crate::time::parse_sqlite_timestamp(&s).map(|t| {
                chrono::Utc::now()
                    .signed_duration_since(t)
                    .num_milliseconds()
                    .max(0) as u64
            })
        })
        .transpose()
        .map_err(|e| AppError::Internal(e.to_string()))?
        .unwrap_or(0);
    Ok(Backlog {
        pending: row.0 as u64,
        bytes: row.1 as u64,
        oldest_millis,
    })
}

/// One bounded batch. No network wait holds the SQLite writer. A lost acknowledgement
/// or failed checkpoint causes safe replay using stable IDs and consumer sequences.
pub async fn publish_batch(
    db: Arc<TenantDb>,
    sink: &dyn EventSink,
    metrics: &Metrics,
) -> Result<(), AppError> {
    let _job = match db.background_jobs() {Some(jobs)=>Some(jobs.acquire(crate::background::Priority::Outbox).await?),None=>None};
    db.ensure_current_owner()?;
    let pool = db.background_read_pool()?;
    let initial: i64 = sqlx::query_scalar(
        "SELECT acknowledged_sequence FROM outbox_publish_state WHERE singleton=1",
    )
    .fetch_one(&pool)
    .await?;
    let rows: Vec<TenantEvent> = sqlx::query_as(
        "SELECT sequence,event_id,location_id,aggregate_type,aggregate_id,event_type,occurred_at,schema_version,deleted,payload,analytics_snapshot FROM outbox_events WHERE sequence>? ORDER BY sequence LIMIT ?"
    ).bind(initial).bind(BATCH_SIZE).fetch_all(&pool).await?;
    let mut acknowledged = initial;
    let mut failure = None;
    for mut event in rows {
        db.ensure_current_owner()?;
        if event.sequence != acknowledged + 1 {
            failure = Some(AppError::Internal("Canonical outbox sequence gap".into()));
            break;
        }
        event.tenant_id = db.tenant_id();
        match sink.publish(&event).await {
            Ok(()) => {
                acknowledged = event.sequence;
                metrics.outbox_published();
            }
            Err(error) => {
                failure = Some(error);
                break;
            }
        }
    }
    let deletable: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM outbox_events WHERE sequence<=? AND sequence<=(SELECT sequence FROM realtime_projection_cursor WHERE singleton=1))"
    ).bind(acknowledged).fetch_one(&pool).await?;
    if acknowledged > initial || deletable {
        db.with_immediate_writer(move |connection| Box::pin(async move {
            sqlx::query("UPDATE outbox_publish_state SET acknowledged_sequence=MAX(acknowledged_sequence,?) WHERE singleton=1")
                .bind(acknowledged).execute(&mut *connection).await?;
            sqlx::query("DELETE FROM outbox_events WHERE sequence<=(SELECT acknowledged_sequence FROM outbox_publish_state WHERE singleton=1) AND sequence<=(SELECT sequence FROM realtime_projection_cursor WHERE singleton=1)")
                .execute(&mut *connection).await?;
            Ok(())
        })).await?;
    }
    if let Some(error) = failure {
        return Err(error);
    }
    Ok(())
}

/// Pending tenants survive idle handle eviction. Empty polling never touches activity,
/// and reopening requires an existing valid lease; publishing cannot acquire one.
pub fn spawn(
    manager: Arc<TenantDbManager>,
    metrics: Arc<Metrics>,
    url: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut sink = None;
        let mut pending = HashSet::new();
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let mut handles = manager.open_handles().await;
            let current: HashSet<_> = handles.iter().map(|db| db.tenant_id()).collect();
            for tenant in pending.difference(&current) {
                if let Ok(db) = manager.open(*tenant).await {
                    handles.push(db);
                }
            }
            let mut total = Backlog::default();
            let mut ready = Vec::new();
            for db in handles {
                match backlog(&db).await {
                    Ok(b) => {
                        total.pending += b.pending;
                        total.bytes += b.bytes;
                        total.oldest_millis = total.oldest_millis.max(b.oldest_millis);
                        if b.pending > 0 {
                            pending.insert(db.tenant_id());
                            ready.push(db);
                        } else {
                            pending.remove(&db.tenant_id());
                        }
                    }
                    Err(error) => tracing::warn!(%error,"Outbox backlog read delayed"),
                }
            }
            metrics.set_publish_backlog(total.pending, total.bytes, total.oldest_millis);
            if ready.is_empty() {
                continue;
            }
            if sink.is_none() {
                match JetStreamSink::connect(&url).await {
                    Ok(connected) => sink = Some(connected),
                    Err(_) => {
                        metrics.outbox_publish_failed();
                        continue;
                    }
                }
            }
            for db in ready {
                if let Err(error) =
                    publish_batch(db.clone(), sink.as_ref().unwrap(), &metrics).await
                {
                    metrics.outbox_publish_failed();
                    tracing::warn!(tenant=%db.tenant_id(),%error,"Tenant outbox publication delayed");
                }
            }
        }
    })
}

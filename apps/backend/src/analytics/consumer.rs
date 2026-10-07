//! One fenced transaction per tenant batch; broker ACKs follow the commit.
use super::{
    publisher::TenantEvent,
    session_hours,
    tenant_db::{error, TenantAnalytics},
};
use crate::{
    error::AppError,
    tenancy::analytics_snapshot::{table, Change, ColumnKind, Table},
};
use chrono_tz::Tz;
use duckdb::{params, params_from_iter, Transaction};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;
pub const BATCH_SIZE: usize = 500;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchOutcome {
    Applied {
        last_sequence: i64,
        events: usize,
        duplicates: usize,
    },
    Gap {
        expected: i64,
        received: i64,
    },
    RebuildRequired,
}
fn invalid(s: &str) -> AppError {
    AppError::Internal(format!("Analytics snapshot: {s}"))
}
fn parse_uuid(s: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(s).map_err(|_| invalid("invalid UUID"))
}
fn timestamp(s: &str) -> Result<chrono::DateTime<chrono::Utc>, AppError> {
    crate::time::parse_sqlite_timestamp(s).map_err(|_| invalid("invalid UTC instant"))
}
fn value(v: &Value, kind: ColumnKind) -> Result<Option<String>, AppError> {
    if v.is_null() {
        return Ok(None);
    }
    let result = match kind {
        ColumnKind::Integer => v
            .as_i64()
            .ok_or_else(|| invalid("invalid integer"))?
            .to_string(),
        ColumnKind::Boolean => v
            .as_bool()
            .ok_or_else(|| invalid("invalid boolean"))?
            .to_string(),
        _ => {
            let s = v.as_str().ok_or_else(|| invalid("invalid text value"))?;
            match kind {
                ColumnKind::Uuid | ColumnKind::StockKey => {
                    parse_uuid(s)?;
                }
                ColumnKind::Timestamp => {
                    timestamp(s)?;
                }
                ColumnKind::Money => {
                    let d = s
                        .parse::<rust_decimal::Decimal>()
                        .map_err(|_| invalid("invalid decimal"))?;
                    crate::tenancy::decimal_to_scale4(d)
                        .map_err(|_| invalid("decimal outside scale-4 range"))?;
                }
                ColumnKind::Date => {
                    s.parse::<chrono::NaiveDate>()
                        .map_err(|_| invalid("invalid calendar date"))?;
                }
                _ => {}
            }
            s.to_owned()
        }
    };
    Ok(Some(result))
}
pub(crate) fn sql_type(kind: ColumnKind) -> &'static str {
    match kind {
        ColumnKind::Uuid | ColumnKind::StockKey => "UUID",
        ColumnKind::Timestamp => "TIMESTAMP",
        ColumnKind::Date => "DATE",
        ColumnKind::Money => "DECIMAL(19,4)",
        ColumnKind::Boolean => "BOOLEAN",
        ColumnKind::Integer => "BIGINT",
        ColumnKind::Text => "VARCHAR",
    }
}
pub fn derive_labels(spec: &Table, row: &mut Value, zone: Tz) -> Result<(), AppError> {
    let pair = match spec.name {
        "transactions" => Some(("local_date", "occurred_at")),
        "sessions" => Some(("start_local_date", "start_time")),
        "expenses" => Some(("local_date", "expense_date")),
        _ => None,
    };
    if let Some((date, time)) = pair {
        row[date] = Value::String(
            timestamp(
                row[time]
                    .as_str()
                    .ok_or_else(|| invalid("missing fact timestamp"))?,
            )?
            .with_timezone(&zone)
            .date_naive()
            .to_string(),
        );
    }
    Ok(())
}
pub fn insert_row(tx: &Transaction<'_>, spec: &Table, row: &Value) -> Result<(), AppError> {
    let object = row
        .as_object()
        .ok_or_else(|| invalid("row is not an object"))?;
    if object.len() != spec.columns.len()
        || spec.columns.iter().any(|c| !object.contains_key(c.name))
    {
        return Err(invalid("column set differs from schema"));
    }
    let columns = spec
        .columns
        .iter()
        .map(|c| c.name)
        .collect::<Vec<_>>()
        .join(",");
    let casts = spec
        .columns
        .iter()
        .map(|c| format!("CAST(? AS {})", sql_type(c.kind)))
        .collect::<Vec<_>>()
        .join(",");
    let values = spec
        .columns
        .iter()
        .map(|c| value(&row[c.name], c.kind))
        .collect::<Result<Vec<_>, _>>()?;
    tx.execute(
        &format!(
            "INSERT INTO {}({columns}) VALUES({casts}) ON CONFLICT(id) DO UPDATE SET {}",
            spec.name,
            spec.columns
                .iter()
                .filter(|c| c.name != "id")
                .map(|c| format!("{}=excluded.{}", c.name, c.name))
                .collect::<Vec<_>>()
                .join(",")
        ),
        params_from_iter(values),
    )
    .map_err(error)?;
    Ok(())
}
pub fn replace_hours(tx: &Transaction<'_>, id: &str, zone: Tz) -> Result<(), AppError> {
    tx.execute(
        "DELETE FROM session_hours WHERE session_id=CAST(? AS UUID)",
        params![id],
    )
    .map_err(error)?;
    let row:Option<(String,String,Option<String>,Option<String>)>=tx.query_row("SELECT CAST(start_time AS VARCHAR),CAST(end_time AS VARCHAR),CAST(device_id AS VARCHAR),CAST(location_id AS VARCHAR) FROM sessions WHERE id=CAST(? AS UUID) AND end_time IS NOT NULL",params![id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(error)?;
    if let Some((start, end, device, location)) = row {
        let parse = |s: &str| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f")
                .map(|t| t.and_utc())
                .map_err(|_| invalid("stored session timestamp"))
        };
        let hot:String=tx.query_row("SELECT CAST(hot_window_start AS VARCHAR) FROM _ingest_state",[],|r|r.get(0)).map_err(error)?;
        let hot=hot.parse::<chrono::NaiveDate>().map_err(|_|invalid("invalid hot date"))?;
        for h in session_hours::split(parse(&start)?, parse(&end)?, zone)? {
            if h.local_date<hot {continue;}
            tx.execute("INSERT INTO session_hours VALUES(CAST(? AS UUID),CAST(? AS TIMESTAMP),CAST(? AS DATE),?,?,?, ?,CAST(? AS UUID),CAST(? AS UUID))",params![id,crate::time::format_sqlite_timestamp(&h.hour_start).map_err(|_|invalid("hour timestamp"))?,h.local_date.to_string(),h.weekday,h.local_hour,h.occupied_seconds,h.is_start_hour,device,location]).map_err(error)?;
        }
    }
    Ok(())
}
use duckdb::OptionalExt;
fn apply_change(tx: &Transaction<'_>, change: Change, zone: Tz, hot: &str) -> Result<(), AppError> {
    let spec = table(&change.table)?;
    if change.key_column != "id" && Some(change.key_column.as_str()) != spec.parent {
        return Err(invalid("invalid replacement key"));
    }
    let key = parse_uuid(&change.key)?.to_string();
    tx.execute(
        &format!(
            "DELETE FROM {} WHERE {}=CAST(? AS UUID)",
            spec.name, change.key_column
        ),
        params![key],
    )
    .map_err(error)?;
    let mut ids = HashSet::new();
    for mut row in change.rows {
        if row[&change.key_column]
            .as_str()
            .map(parse_uuid)
            .transpose()?
            != Some(parse_uuid(&key)?)
        {
            return Err(invalid("foreign replacement row"));
        }
        let id = parse_uuid(
            row["id"]
                .as_str()
                .ok_or_else(|| invalid("missing row id"))?,
        )?;
        if !ids.insert(id) {
            return Err(invalid("duplicate replacement row"));
        }
        derive_labels(spec, &mut row, zone)?;
        if let Some(time) = spec.time {
            if let Some(instant)=row[time].as_str() {
                let before=timestamp(instant)?.with_timezone(&zone).date_naive().to_string().as_str()<hot;
                if before {
                    let end=match spec.name {"sessions"=>Some("end_time"),"shifts"=>Some("clock_out"),_=>None};
                    let overlaps=match end {
                        Some(end) if row[end].is_null()=>true,
                        Some(end)=>timestamp(row[end].as_str().ok_or_else(||invalid("invalid interval end"))?)?>super::rebuild::boundary(hot.parse().map_err(|_|invalid("invalid hot date"))?,zone)?,
                        None=>false,
                    };
                    if !overlaps {continue;}
                }
            } else if !row[time].is_null() {return Err(invalid("invalid retention timestamp"));}
        }
        if let Some(parent) = spec.parent {
            let parent_table = match spec.name {
                "transaction_lines" => "transactions",
                "credit_settlement_items" => "credit_settlements",
                "stock_receipt_lines" => "stock_receipts",
                "stock_waste_lines" => "stock_waste_events",
                _ => return Err(invalid("unknown parent")),
            };
            let present: bool = tx
                .query_row(
                    &format!(
                        "SELECT EXISTS(SELECT 1 FROM {parent_table} WHERE id=CAST(? AS UUID))"
                    ),
                    params![row[parent].as_str()],
                    |r| r.get(0),
                )
                .map_err(error)?;
            if !present {
                continue;
            }
        }
        insert_row(tx, spec, &row)?;
    }
    if spec.name == "sessions" {
        replace_hours(tx, &key, zone)?;
    }
    Ok(())
}
/// Sorts replay within a bounded batch, validates all sequences before mutating
/// facts, and collapses independent row/parent replacements to their last state.
pub async fn apply_batch(
    analytics: Arc<TenantAnalytics>,
    mut events: Vec<TenantEvent>,
) -> Result<BatchOutcome, AppError> {
    if events.len() > 1000 {
        return Err(invalid("batch exceeds bounded capacity"));
    }
    let tenant = analytics.tenant_id();
    analytics.write(move|tx|{
  let (mut last,status,timezone,hot):(i64,String,String,String)=tx.query_row("SELECT CAST(last_sequence AS BIGINT),status,timezone,CAST(hot_window_start AS VARCHAR) FROM _ingest_state WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(error)?;
  let initial=last;let zone=timezone.parse::<Tz>().map_err(|_|invalid("invalid tenant timezone"))?;
  if status!="READY" && status!="LAGGING" {return Ok(BatchOutcome::RebuildRequired);}
  events.sort_by_key(|e|e.sequence);
  let mut changes=BTreeMap::new();let mut duplicates=0;let mut previous:Option<(i64,String)>=None;
  for event in &events {
   if event.tenant_id!=tenant{return Err(invalid("foreign tenant envelope"));}
   if event.sequence<=0 || event.schema_version!=1{return Err(invalid("unsupported event sequence/version"));}
   parse_uuid(&event.event_id)?;parse_uuid(&event.aggregate_id)?;timestamp(&event.occurred_at)?;
   if event.sequence<=last {
    if let Some((seq,id))=&previous {if *seq==event.sequence && id!=&event.event_id{return Err(invalid("conflicting sequence identity"));}}
    duplicates+=1;continue;
   }
   if event.sequence!=last+1 {tx.execute("UPDATE _ingest_state SET status='LAGGING',updated_at=CAST(? AS TIMESTAMP) WHERE id=1",params![crate::time::format_sqlite_timestamp(&chrono::Utc::now()).map_err(|_|invalid("checkpoint timestamp"))?]).map_err(error)?;return Ok(BatchOutcome::Gap{expected:last+1,received:event.sequence});}
   let Some(snapshot)=&event.analytics_snapshot else {tx.execute_batch("UPDATE _ingest_state SET status='REBUILDING'").map_err(error)?;return Ok(BatchOutcome::RebuildRequired);};
   if snapshot.version!=1{return Err(invalid("unsupported projection version"));}
   for change in &snapshot.changes {changes.insert((change.table.clone(),change.key_column.clone(),change.key.clone()),(event.sequence,change.clone()));}
   last=event.sequence;previous=Some((last,event.event_id.clone()));
  }
  // Apply parent replacements before children; row replacements follow event order.
  let mut changes=changes.into_values().collect::<Vec<_>>();
  changes.sort_by_key(|(seq,c)|(table(&c.table).map(|s|s.parent.is_some()).unwrap_or(false),*seq));
  for (_,change) in changes {apply_change(tx,change,zone,&hot)?;}
  if last>initial {
   let event=events.iter().rev().find(|e|e.sequence==last).unwrap();
   tx.execute("UPDATE _ingest_state SET last_sequence=?,last_event_at=CAST(? AS TIMESTAMP),updated_at=CAST(? AS TIMESTAMP),status='READY' WHERE id=1",params![last,event.occurred_at,crate::time::format_sqlite_timestamp(&chrono::Utc::now()).map_err(|_|invalid("checkpoint timestamp"))?]).map_err(error)?;
  }
  Ok(BatchOutcome::Applied{last_sequence:last,events:(last-initial) as usize,duplicates})
 }).await
}

use crate::{
    metrics::Metrics,
    tenancy::{TenantDb, TenantDbManager},
};
use async_nats::jetstream::{
    consumer::{pull, AckPolicy, DeliverPolicy, PullConsumer},
    message::AckKind,
};
use futures::StreamExt;
use std::time::Duration;
/// One durable filtered consumer per tenant. Buffer lifetime is at most one second;
/// no SQLite writer is held while waiting for messages or broker acknowledgements.
pub struct JetStreamConsumer {
    db: Arc<TenantDb>,
    analytics: Arc<TenantAnalytics>,
    consumer: PullConsumer,
    gap_attempts: usize,
}
impl JetStreamConsumer {
    pub async fn connect(
        context: &async_nats::jetstream::Context,
        db: Arc<TenantDb>,
        analytics: Arc<TenantAnalytics>,
    ) -> Result<Self, AppError> {
        db.ensure_current_owner()?;
        if db.tenant_id() != analytics.tenant_id() {
            return Err(invalid("foreign analytics handle"));
        }
        let stream = context
            .get_stream(super::publisher::TENANT_EVENT_STREAM)
            .await
            .map_err(|_| invalid("replay stream unavailable"))?;
        let subject = format!("arena.tenant.{}.events.v1", db.tenant_id());
        let name = format!("analytics_{}", db.tenant_id().simple());
        let mut consumer = stream
            .get_or_create_consumer(
                &name,
                pull::Config {
                    durable_name: Some(name.clone()),
                    filter_subject: subject.clone(),
                    deliver_policy: DeliverPolicy::All,
                    ack_policy: AckPolicy::Explicit,
                    ack_wait: Duration::from_secs(30),
                    max_ack_pending: 1000,
                    max_waiting: 1,
                    max_batch: 1000,
                    ..Default::default()
                },
            )
            .await
            .map_err(|_| invalid("consumer setup failed"))?;
        let config = &consumer
            .info()
            .await
            .map_err(|_| invalid("consumer verification failed"))?
            .config;
        if config.filter_subject != subject
            || config.ack_policy != AckPolicy::Explicit
            || config.max_ack_pending <= 0
            || config.max_ack_pending > 1000
            || config.deliver_policy != DeliverPolicy::All
        {
            return Err(invalid("incompatible existing consumer"));
        }
        db.ensure_current_owner()?;
        Ok(Self {
            db,
            analytics,
            consumer,
            gap_attempts: 0,
        })
    }
    pub async fn poll(&mut self, metrics: &Metrics) -> Result<BatchOutcome, AppError> {
        self.db.ensure_current_owner()?;
        let (initial_sequence, status, replay_start) = self
            .analytics
            .read(|tx| {
                tx.query_row(
                    "SELECT CAST(last_sequence AS BIGINT),status,replay_start_sequence FROM _ingest_state",
                    [],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_,Option<u64>>(2)?)),
                )
                .map_err(error)
            })
            .await?;
        if status != "READY" && status != "LAGGING" {
            return Ok(BatchOutcome::RebuildRequired);
        }
        let mut batch = self
            .consumer
            .batch()
            .max_messages(BATCH_SIZE)
            .max_bytes(8 * 1024 * 1024)
            .expires(Duration::from_secs(1))
            .messages()
            .await
            .map_err(|_| invalid("batch fetch failed"))?;
        let mut messages = Vec::with_capacity(BATCH_SIZE);
        let mut events = Vec::with_capacity(BATCH_SIZE);
        let mut covered=0;
        while let Some(message) = batch.next().await {
            let message = message.map_err(|_| invalid("message delivery failed"))?;
            if message.subject.as_str() != format!("arena.tenant.{}.events.v1", self.db.tenant_id())
            {
                return Err(invalid("foreign event subject"));
            }
            if let Some(start)=replay_start {
                let info=message.info().map_err(|_|invalid("invalid broker metadata"))?;
                if info.stream!=super::publisher::TENANT_EVENT_STREAM {return Err(invalid("foreign broker stream"));}
                if info.stream_sequence<start {
                    covered+=1;
                    messages.push(message);
                    continue;
                }
            }
            events.push(
                serde_json::from_slice::<TenantEvent>(&message.payload)
                    .map_err(|_| invalid("invalid event envelope"))?,
            );
            messages.push(message);
        }
        self.db.ensure_current_owner()?;
        if messages.is_empty() {
            return Ok(BatchOutcome::Applied {
                last_sequence: initial_sequence,
                events: 0,
                duplicates: 0,
            });
        }
        static COMMITS: std::sync::LazyLock<tokio::sync::Semaphore> =
            std::sync::LazyLock::new(|| tokio::sync::Semaphore::new(4));
        let permit = COMMITS
            .acquire()
            .await
            .map_err(|_| invalid("commit scheduler closed"))?;
        let mut result = apply_batch(self.analytics.clone(), events).await?;
        if let BatchOutcome::Applied{duplicates,..}=&mut result {*duplicates+=covered;}
        drop(permit);
        match &result {
            BatchOutcome::Applied { events, .. } => {
                self.gap_attempts = 0;
                metrics.analytics_ingested(*events as u64);
                for message in messages {
                    self.db.ensure_current_owner()?;
                    message
                        .double_ack()
                        .await
                        .map_err(|_| invalid("post-commit acknowledgement failed"))?;
                }
            }
            BatchOutcome::Gap { .. } => {
                self.gap_attempts += 1;
                metrics.analytics_gap();
                // Re-fetch unacknowledged deliveries first. Persistent gaps request the
                // consistent snapshot rebuild.
                if self.gap_attempts >= 2 {
                    self.analytics
                        .write(|tx| {
                            tx.execute_batch("UPDATE _ingest_state SET status='REBUILDING'")
                                .map_err(error)?;
                            Ok(())
                        })
                        .await?;
                }
                for message in messages {
                    self.db.ensure_current_owner()?;
                    message
                        .ack_with(AckKind::Nak(Some(Duration::from_millis(250))))
                        .await
                        .map_err(|_| invalid("gap re-fetch request failed"))?;
                }
            }
            BatchOutcome::RebuildRequired => {}
        }
        Ok(result)
    }
}
/// Tenant workers are independent so one tenant's one-second buffer does not
/// delay another tenant. Commits are bounded per cell; idle/fenced workers close.
pub fn spawn(
    manager: Arc<TenantDbManager>,
    metrics: Arc<Metrics>,
    url: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut context = None;
        let mut workers =
            std::collections::HashMap::<Uuid, (i64, tokio::task::JoinHandle<()>)>::new();
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            let handles = manager.open_handles().await;
            let active = handles
                .iter()
                .map(|db| (db.tenant_id(), db.ownership_generation()))
                .collect::<std::collections::HashMap<_, _>>();
            workers.retain(|tenant, (generation, worker)| {
                if active.get(tenant) != Some(generation) || worker.is_finished() {
                    worker.abort();
                    false
                } else {
                    true
                }
            });
            if handles.is_empty() {
                continue;
            }
            if context.is_none() {
                let client =
                    match tokio::time::timeout(Duration::from_secs(1), async_nats::connect(&url))
                        .await
                    {
                        Ok(Ok(c)) => c,
                        _ => {
                            metrics.analytics_failed();
                            continue;
                        }
                    };
                let mut c = async_nats::jetstream::new(client);
                c.set_timeout(Duration::from_secs(1));
                context = Some(c);
            }
            for db in handles {
                if workers.contains_key(&db.tenant_id()) {
                    continue;
                }
                let context = context.as_ref().unwrap().clone();
                let metrics = metrics.clone();
                let id = db.tenant_id();
                let generation = db.ownership_generation();
                let worker = tokio::spawn(async move {
                    let result = async {
                        let analytics = TenantAnalytics::open_for_ingestion(db.clone()).await?;
                        let mut consumer =
                            JetStreamConsumer::connect(&context, db.clone(), analytics).await?;
                        let mut retention_due=chrono::Utc::now();
                        loop {
                            db.ensure_current_owner()?;
                            let now=chrono::Utc::now();
                            if now>=retention_due {
                                match super::retention::run(consumer.analytics.clone(),now).await {
                                    Ok(super::retention::RetentionOutcome::Applied{deleted_rows,next_due,..})=> {
                                        metrics.analytics_retained(deleted_rows as u64);retention_due=next_due;
                                    }
                                    Ok(super::retention::RetentionOutcome::Skipped)=>retention_due=now+chrono::Duration::seconds(5),
                                    Err(error)=> {metrics.analytics_failed();tracing::warn!(tenant=%id,%error,"Analytics retention delayed");retention_due=now+chrono::Duration::seconds(5);}
                                }
                            }
                            match consumer.poll(&metrics).await {
                                Ok(BatchOutcome::RebuildRequired) => {
                                    metrics.analytics_rebuild_started();
                                    let started=std::time::Instant::now();
                                    let result=super::rebuild::rebuild(db.clone(),consumer.analytics.clone(),&context).await;
                                    metrics.analytics_rebuild_finished(result.is_ok(),started.elapsed().as_millis().min(u64::MAX as u128) as u64);
                                    if let Err(error)=result {
                                        metrics.analytics_failed();
                                        tracing::warn!(tenant=%id,%error,"Analytics rebuild delayed");
                                        tokio::time::sleep(Duration::from_secs(5)).await;
                                    }
                                }
                                Ok(_) => {}
                                Err(error) => {
                                    metrics.analytics_failed();
                                    tracing::warn!(tenant=%id,%error,"Analytics ingestion delayed");
                                    if matches!(&error,AppError::Internal(message) if message.starts_with("DuckDB:")) {return Err(error);}
                                    tokio::time::sleep(Duration::from_secs(1)).await;
                                }
                            }
                        }
                        #[allow(unreachable_code)]
                        Ok::<(), AppError>(())
                    }
                    .await;
                    if let Err(error) = result {
                        metrics.analytics_failed();
                        tracing::warn!(tenant=%id,%error,"Analytics worker closed");
                    }
                });
                workers.insert(id, (generation, worker));
            }
        }
    })
}

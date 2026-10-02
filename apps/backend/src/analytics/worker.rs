//! One process owns the relay and one durable pull consumer. Never ACK before a
//! synchronous ClickHouse insert succeeds. Replacement versions make replay safe.
use super::{client::unavailable, ClickHouse};
use async_nats::jetstream::{
    self,
    consumer::{pull, AckPolicy},
    stream::{Config, DiscardPolicy, RetentionPolicy, StorageType},
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{Connection, PgConnection, PgPool};
use std::{collections::BTreeMap, time::Duration};
use uuid::Uuid;

type Error = Box<dyn std::error::Error + Send + Sync>;
const STREAM: &str = "ARENA360_ANALYTICS";
const SUBJECT: &str = "arena360.analytics.v1.rows";
const CONSUMER: &str = "clickhouse-v1";
const LOCK: i64 = 360_20261003;

#[derive(Debug, Serialize, Deserialize)]
pub struct Change {
    pub schema_version: u8,
    pub source_table: String,
    pub row_id: Uuid,
    pub version: u64,
    pub deleted: bool,
    pub row_data: Value,
}

pub fn schema() -> &'static BTreeMap<String, BTreeMap<String, String>> {
    static SCHEMA: std::sync::LazyLock<BTreeMap<String, BTreeMap<String, String>>> =
        std::sync::LazyLock::new(|| {
            serde_json::from_str(include_str!("schema.json")).expect("checked-in analytics schema")
        });
    &SCHEMA
}

pub fn project(change: &Change) -> Result<Value, crate::error::AppError> {
    if change.schema_version != 1 {
        return Err(unavailable("Unsupported analytics schema version"));
    }
    let schema = schema();
    let columns = schema
        .get(&change.source_table)
        .ok_or_else(|| unavailable("Unknown analytics source"))?;
    if change.row_data.get("id").and_then(Value::as_str) != Some(change.row_id.to_string().as_str())
    {
        return Err(unavailable("Analytics row identity mismatch"));
    }
    let mut row = Map::new();
    for (name, kind) in columns {
        let value = change.row_data.get(name).cloned().unwrap_or(Value::Null);
        if value.is_null() && !kind.starts_with("Nullable(") {
            return Err(unavailable(format!(
                "Missing required analytics column: {}.{name}",
                change.source_table
            )));
        }
        row.insert(name.clone(), value);
    }
    row.insert("_version".into(), change.version.into());
    row.insert("_deleted".into(), u8::from(change.deleted).into());
    Ok(Value::Object(row))
}

async fn backfill(pool: &PgPool) -> Result<(), Error> {
    for (table, columns) in schema() {
        let mut cursor: Option<Uuid> = None;
        loop {
            // Version zero is older than every trigger event, even if the event
            // was delivered before this snapshot. No timestamp ordering races.
            let names = columns
                .keys()
                .map(|s| format!("'{s}'"))
                .collect::<Vec<_>>()
                .join(",");
            let source = if table == "location_stock" {
                "(SELECT s.*, md5(s.\"locationId\"::text || ':' || s.\"productId\"::text)::uuid AS id FROM location_stock s)".to_string()
            } else {
                table.clone()
            };
            let sql = format!(
                r#"WITH batch AS (
              SELECT id, (SELECT jsonb_object_agg(key,value) FROM jsonb_each(to_jsonb(t))
                WHERE key IN ({names})) AS payload FROM {source} t WHERE ($1::uuid IS NULL OR id > $1) ORDER BY id LIMIT 1000
            ), inserted AS (
              INSERT INTO analytics_outbox(source_table,row_id,row_data,deleted,version)
              SELECT $2,id,payload,false,0 FROM batch RETURNING row_id
            ) SELECT row_id FROM inserted ORDER BY row_id DESC LIMIT 1"#
            );
            let last: Option<Uuid> = sqlx::query_scalar(&sql)
                .bind(cursor)
                .bind(table)
                .fetch_optional(pool)
                .await?;
            match last {
                Some(id) => cursor = Some(id),
                None => break,
            }
        }
        tracing::info!(%table, "Analytics snapshot enqueued");
    }
    sqlx::query("INSERT INTO analytics_outbox(source_table,row_id,row_data,version) VALUES('__ready', $1, '{}', 0)")
        .bind(Uuid::nil()).execute(pool).await?;
    Ok(())
}

async fn relay(pool: &PgPool, js: &jetstream::Context, ch: &ClickHouse) -> Result<(), Error> {
    loop {
        let rows: Vec<(i64, String, Uuid, Value, bool, Option<i64>)> = sqlx::query_as(
            "SELECT id,source_table,row_id,row_data,deleted,version FROM analytics_outbox ORDER BY id LIMIT 500"
        ).fetch_all(pool).await?;
        if rows.is_empty() {
            tokio::time::sleep(Duration::from_millis(250)).await;
            continue;
        }
        // The backfill barrier is processed only after all earlier messages
        // are acknowledged, including deliveries pending after a crash.
        if rows[0].1 == "__ready" || rows[0].1 == "__ready_business" {
            let stream = js.get_stream(STREAM).await?;
            let mut consumer: jetstream::consumer::Consumer<pull::Config> =
                stream.get_consumer(CONSUMER).await?;
            loop {
                let info = consumer.info().await?;
                if info.num_pending == 0 && info.num_ack_pending == 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            // A complete backfill includes the business projections. The
            // upgrade barrier marks only those projections as ready.
            let ready_sql = if rows[0].1 == "__ready" {
                "INSERT INTO analytics_ready VALUES (1, now64(6)), (2, now64(6))"
            } else {
                "INSERT INTO analytics_ready VALUES (2, now64(6))"
            };
            ch.execute(ready_sql, &[]).await?;
            sqlx::query("DELETE FROM analytics_outbox WHERE id=$1")
                .bind(rows[0].0)
                .execute(pool)
                .await?;
            continue;
        }
        let mut acknowledgments = Vec::new();
        let mut ids = Vec::new();
        for (id, source_table, row_id, row_data, deleted, version) in rows
            .into_iter()
            .take_while(|r| r.1 != "__ready" && r.1 != "__ready_business")
        {
            let change = Change {
                schema_version: 1,
                source_table,
                row_id,
                row_data,
                deleted,
                version: version.unwrap_or(id) as u64,
            };
            let mut headers = async_nats::HeaderMap::new();
            headers.insert("Nats-Msg-Id", format!("analytics-v1-{id}"));
            acknowledgments.push(
                js.publish_with_headers(SUBJECT, headers, serde_json::to_vec(&change)?.into())
                    .await?,
            );
            ids.push(id);
        }
        // Pipeline broker acknowledgments rather than paying one network RTT
        // per event. Partial success is safely replayed with identical versions.
        futures::future::try_join_all(
            acknowledgments
                .into_iter()
                .map(std::future::IntoFuture::into_future),
        )
        .await?;
        sqlx::query("DELETE FROM analytics_outbox WHERE id=ANY($1)")
            .bind(&ids)
            .execute(pool)
            .await?;
    }
}

async fn consume(
    consumer: jetstream::consumer::Consumer<pull::Config>,
    ch: &ClickHouse,
) -> Result<(), Error> {
    loop {
        let mut messages = consumer
            .fetch()
            .max_messages(500)
            .expires(Duration::from_secs(1))
            .messages()
            .await?;
        let mut pending = vec![];
        let mut batches: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        while let Some(message) = messages.next().await {
            let message = message?;
            let change: Change = serde_json::from_slice(&message.payload)?;
            batches
                .entry(change.source_table.clone())
                .or_default()
                .push(project(&change)?);
            pending.push(message);
        }
        for (table, rows) in batches {
            let body = rows
                .iter()
                .map(serde_json::to_string)
                .collect::<Result<Vec<_>, _>>()?
                .join("\n");
            ch.execute(
                &format!("INSERT INTO {table}_versions FORMAT JSONEachRow\n{body}"),
                &[],
            )
            .await?;
        }
        for message in pending {
            message.double_ack().await?;
        }
    }
}

pub async fn run(initial_backfill: bool) -> Result<(), Error> {
    crate::config::load_dotenv();
    // Session-level advisory locks require a direct PostgreSQL connection.
    let direct_url = std::env::var("ANALYTICS_DATABASE_URL")
        .map_err(|_| "ANALYTICS_DATABASE_URL must point directly at PostgreSQL (not transaction-pooled PgBouncer)")?;
    let mut lease = PgConnection::connect(&direct_url).await?;
    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(LOCK)
        .fetch_one(&mut lease)
        .await?;
    if !acquired {
        return Err("Another analytics worker owns the singleton lock".into());
    }
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(3)
        .connect(&direct_url)
        .await?;
    let ch = ClickHouse::from_env();
    ch.initialize().await?;
    let nats = async_nats::connect(
        std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into()),
    )
    .await?;
    let js = jetstream::new(nats);
    let stream = js
        .get_or_create_stream(Config {
            name: STREAM.into(),
            subjects: vec![SUBJECT.into()],
            storage: StorageType::File,
            retention: RetentionPolicy::WorkQueue,
            discard: DiscardPolicy::New,
            max_bytes: 10 * 1024 * 1024 * 1024,
            num_replicas: std::env::var("NATS_ANALYTICS_REPLICAS")
                .unwrap_or_else(|_| "1".into())
                .parse()?,
            ..Default::default()
        })
        .await?;
    let consumer = stream
        .get_or_create_consumer(
            CONSUMER,
            pull::Config {
                durable_name: Some(CONSUMER.into()),
                filter_subject: SUBJECT.into(),
                ack_policy: AckPolicy::Explicit,
                ack_wait: Duration::from_secs(900),
                max_ack_pending: 500,
                max_deliver: -1,
                ..Default::default()
            },
        )
        .await?;
    if initial_backfill {
        backfill(&pool).await?;
    }
    let heartbeat = async {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            sqlx::query("SELECT 1").execute(&mut lease).await?;
        }
        #[allow(unreachable_code)]
        Ok::<(), Error>(())
    };
    // Fail closed on poison events/insert failures; the process supervisor
    // restarts this worker, and unacknowledged messages remain in JetStream.
    tokio::select! {
        result = relay(&pool, &js, &ch) => result,
        result = consume(consumer, &ch) => result,
        result = heartbeat => result,
        _ = shutdown() => Ok(()),
    }
}

async fn shutdown() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("SIGTERM handler");
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = term.recv() => {} }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

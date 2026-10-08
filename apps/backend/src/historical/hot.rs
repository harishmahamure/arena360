//! JetStream-triggered, bounded-memory monthly copies of the SQLite projection.
use super::objects::{self, Object};
use crate::{
    analytics::{
        calendar,
        publisher::{TenantEvent, TENANT_EVENT_STREAM},
    },
    error::AppError,
    replication::{crypto::TenantKeys, ledger::PostgresLedger, snapshot},
    tenancy::{
        analytics_snapshot::{normalize, Table, SNAPSHOT_VERSION, TABLES},
        TenantDb, TenantDbManager,
    },
};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use chrono_tz::Tz;
use futures::TryStreamExt;
use object_store::ObjectStoreExt;
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;
fn fail(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("Hot Parquet: {e}"))
}
fn literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
pub fn month(date: NaiveDate, offset: i32) -> Result<NaiveDate, AppError> {
    let n = date.year() * 12 + date.month0() as i32 + offset;
    NaiveDate::from_ymd_opt(n.div_euclid(12), n.rem_euclid(12) as u32 + 1, 1)
        .ok_or_else(|| fail("Invalid month"))
}
struct Files(PathBuf);
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn files(root: &Path) -> Result<Files, AppError> {
    let path = root.join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&path).map_err(fail)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).map_err(fail)?;
    }
    Ok(Files(path))
}
fn source_select(spec: &Table, start: &str, end: &str) -> Result<String, AppError> {
    let mut query = spec.select();
    if let Some(time) = spec.time {
        let column = spec
            .columns
            .iter()
            .find(|c| c.name == time)
            .ok_or_else(|| fail("Missing period column"))?;
        query.push_str(&format!(
            " AND {}>={} AND {}<{}",
            column.source,
            literal(start),
            column.source,
            literal(end)
        ));
    }
    if let Some(key) = spec.parent {
        let name = match spec.name {
            "transaction_lines" => "transactions",
            "credit_settlement_items" => "credit_settlements",
            "stock_receipt_lines" => "stock_receipts",
            "stock_waste_lines" => "stock_waste_events",
            _ => return Err(fail("Unknown fact family")),
        };
        let parent = TABLES
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| fail("Missing parent"))?;
        let time = parent.time.ok_or_else(|| fail("Missing parent period"))?;
        let column = parent
            .columns
            .iter()
            .find(|c| c.name == time)
            .ok_or_else(|| fail("Missing parent column"))?;
        query.push_str(&format!(
            " AND r.{key} IN (SELECT r.id FROM {} WHERE {} AND {}>={} AND {}<{})",
            parent.source,
            parent.predicate,
            column.source,
            literal(start),
            column.source,
            literal(end)
        ));
    }
    query.push_str(" ORDER BY json_extract(payload,'$.id')");
    Ok(query)
}
pub(crate) async fn parquet(
    source: &sqlx::SqlitePool,
    spec: &'static Table,
    start: &str,
    end: &str,
    zone: Tz,
    root: &Path,
) -> Result<(PathBuf, i64, String), AppError> {
    let raw = root.join(format!("{}.ndjson", spec.name));
    let mut file = File::create(&raw).map_err(fail)?;
    let mut count = 0i64;
    let select = source_select(spec, start, end)?;
    let mut rows = sqlx::query_scalar::<_, String>(&select).fetch(source);
    while let Some(value) = rows.try_next().await? {
        let mut row = normalize(spec, serde_json::from_str(&value).map_err(fail)?)?;
        crate::analytics::consumer::derive_labels(spec, &mut row, zone)?;
        serde_json::to_writer(&mut file, &row).map_err(fail)?;
        file.write_all(b"\n").map_err(fail)?;
        count += 1;
    }
    file.sync_all().map_err(fail)?;
    drop(file);
    let source_checksum = snapshot::hash_file(&raw)?;
    let path = root.join(format!("{}.parquet", spec.name));
    let output = path.clone();
    let temp = root.join("duck-temp");
    tokio::task::spawn_blocking(move || {
        let connection = duckdb::Connection::open_in_memory().map_err(fail)?;
        connection
            .execute_batch(&format!(
                "SET threads=1; SET memory_limit='128MB'; SET temp_directory={}",
                literal(
                    temp.to_str()
                        .ok_or_else(|| fail("Invalid temporary path"))?
                )
            ))
            .map_err(fail)?;
        let columns = spec
            .columns
            .iter()
            .map(|c| {
                format!(
                    "{} {}",
                    c.name,
                    crate::analytics::consumer::sql_type(c.kind)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        connection
            .execute_batch(&format!("CREATE TABLE facts({columns})"))
            .map_err(fail)?;
        if count > 0 {
            let casts = spec
                .columns
                .iter()
                .map(|c| {
                    format!(
                        "CAST(json_extract_string(json,'$.{}') AS {})",
                        c.name,
                        crate::analytics::consumer::sql_type(c.kind)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            connection
                .execute_batch(&format!(
                    "INSERT INTO facts SELECT {casts} FROM read_ndjson_objects({})",
                    literal(raw.to_str().ok_or_else(|| fail("Invalid JSON path"))?)
                ))
                .map_err(fail)?;
        }
        connection
            .execute_batch(&format!(
                "COPY (SELECT * FROM facts ORDER BY id) TO {} (FORMAT PARQUET,COMPRESSION ZSTD)",
                literal(
                    output
                        .to_str()
                        .ok_or_else(|| fail("Invalid Parquet path"))?
                )
            ))
            .map_err(fail)?;
        let checked: i64 = connection
            .query_row(
                &format!(
                    "SELECT count(*) FROM read_parquet({})",
                    literal(output.to_str().unwrap())
                ),
                [],
                |r| r.get(0),
            )
            .map_err(fail)?;
        if checked != count {
            return Err(fail("Parquet row count differs from SQLite"));
        }
        Ok::<_, AppError>(())
    })
    .await
    .map_err(fail)??;
    Ok((path, count, source_checksum))
}
pub struct Writer {
    pub metrics: Arc<crate::metrics::Metrics>,
    pub ledger: Arc<PostgresLedger>,
    pub store: Arc<dyn object_store::ObjectStore>,
    pub keys: TenantKeys,
    pub staging_root: PathBuf,
}
impl Writer {
    pub async fn refresh(&self, db: Arc<TenantDb>, now: DateTime<Utc>) -> Result<i64, AppError> {
        let started = std::time::Instant::now();
        let result = self.refresh_inner(db, now).await;
        self.metrics.historical_finished(
            "hot_parquet",
            result.is_ok(),
            started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        );
        result
    }
    async fn refresh_inner(&self, db: Arc<TenantDb>, now: DateTime<Utc>) -> Result<i64, AppError> {
        db.ensure_current_owner()?;
        let mut serial = self.ledger.pool.begin().await?;
        let locked: bool =
            sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
                .bind(format!("hot-parquet:{}", db.tenant_id()))
                .fetch_one(&mut *serial)
                .await?;
        if !locked {
            return Err(AppError::Conflict("Hot copy already running".into()));
        }
        let snapshot_permit = match db.background_jobs() {
            Some(j) => Some(j.acquire(crate::background::Priority::HotBackfill).await?),
            None => None,
        };
        let key = self.keys.read(db.tenant_id())?;
        let files = files(&self.staging_root)?;
        let image = files.0.join("source.sqlite");
        sqlx::query("VACUUM INTO ?")
            .bind(
                image
                    .to_str()
                    .ok_or_else(|| fail("Invalid snapshot path"))?,
            )
            .execute(&db.background_read_pool()?)
            .await?;
        db.ensure_current_owner()?;
        drop(snapshot_permit);
        let source = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&image)
                    .read_only(true),
            )
            .await?;
        let result=async{
   let (watermark,timezone):(i64,String)=sqlx::query_as("SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='outbox_events'),0),timezone FROM tenant_runtime WHERE singleton=1").fetch_one(&source).await?;
   let zone=timezone.parse::<Tz>().map_err(fail)?;let current=month(now.with_timezone(&zone).date_naive(),0)?;let hot=month(current,-18)?;
   for offset in -18..=0 {
    db.ensure_current_owner()?;let start=month(current,offset)?;let end=month(start,1)?;
    let mut tx=self.ledger.pool.begin().await?;super::handoff::lock_month(&mut tx,db.tenant_id(),start,true).await?;
    let archived:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM archive_manifests WHERE tenant_id=$1 AND period_start=$2 AND state IN ('VERIFIED','PURGING','COMPLETE'))").bind(db.tenant_id()).bind(start).fetch_one(&self.ledger.pool).await?;if archived{continue;}
    let old:Option<(serde_json::Value,i64)>=sqlx::query_as("SELECT objects,projection_version FROM hot_month_manifests WHERE tenant_id=$1 AND period_start=$2 AND state='READY' AND timezone=$3").bind(db.tenant_id()).bind(start).bind(&timezone).fetch_optional(&self.ledger.pool).await?;
    let old=old.filter(|(_,v)|*v==SNAPSHOT_VERSION as i64).map(|(v,_)|serde_json::from_value::<Vec<Object>>(v).map_err(fail)).transpose()?.unwrap_or_default();
    let from=crate::time::format_sqlite_timestamp(&calendar::boundary(start,zone)?).map_err(fail)?;let to=crate::time::format_sqlite_timestamp(&calendar::boundary(end,zone)?).map_err(fail)?;
    let mut objects=vec![];
    for spec in TABLES {
     let _table_permit=match db.background_jobs(){Some(j)=>Some(j.acquire(crate::background::Priority::HotBackfill).await?),None=>None};
     let (path,rows,hash)=parquet(&source,spec,&from,&to,zone,&files.0).await?;
     let prior=old.iter().find(|o|o.table==spec.name&&o.source_checksum==hash&&o.rows==rows);
     let reusable=if let Some(prior)=prior {self.store.head(&object_store::path::Path::from(prior.key.as_str())).await.is_ok_and(|meta|meta.size==prior.bytes)}else{false};
     let object=if reusable {prior.unwrap().clone()}else{
      let object_key=format!("tenants/{}/hot/{}/{:02}/{}-{}.parquet",db.tenant_id(),start.year(),start.month(),spec.name,Uuid::new_v4());
      objects::upload(self.store.as_ref(),&key,object_key,spec.name.into(),rows,hash,&path).await?
     };objects.push(object);
     for extension in ["parquet","ndjson","encrypted"]{let _=std::fs::remove_file(files.0.join(format!("{}.{}",spec.name,extension)));}
     tokio::task::yield_now().await;
    }
    self.ledger.lock_owner(&db,&mut tx).await?;
    let archived:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM archive_manifests WHERE tenant_id=$1 AND period_start=$2 AND state IN ('VERIFIED','PURGING','COMPLETE'))").bind(db.tenant_id()).bind(start).fetch_one(&mut *tx).await?;
    if !archived{
     sqlx::query("INSERT INTO hot_month_manifests(tenant_id,period_start,period_end,timezone,ownership_generation,source_watermark,projection_version,objects) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(tenant_id,period_start) DO UPDATE SET timezone=EXCLUDED.timezone,ownership_generation=EXCLUDED.ownership_generation,source_watermark=EXCLUDED.source_watermark,projection_version=EXCLUDED.projection_version,objects=EXCLUDED.objects,verified_at=clock_timestamp() WHERE hot_month_manifests.state='READY'").bind(db.tenant_id()).bind(start).bind(end).bind(&timezone).bind(db.ownership_generation()).bind(watermark).bind(SNAPSHOT_VERSION as i64).bind(serde_json::to_value(&objects).map_err(fail)?).execute(&mut *tx).await?;
    }
    tx.commit().await?;
   }
   let mut tx=self.ledger.pool.begin().await?;self.ledger.lock_owner(&db,&mut tx).await?;
   sqlx::query("INSERT INTO hot_tenant_state(tenant_id,ownership_generation,source_watermark,timezone,hot_window_start,next_due) VALUES($1,$2,$3,$4,$5,$6+INTERVAL '1 day') ON CONFLICT(tenant_id) DO UPDATE SET ownership_generation=EXCLUDED.ownership_generation,source_watermark=EXCLUDED.source_watermark,timezone=EXCLUDED.timezone,hot_window_start=EXCLUDED.hot_window_start,next_due=EXCLUDED.next_due,updated_at=clock_timestamp()").bind(db.tenant_id()).bind(db.ownership_generation()).bind(watermark).bind(&timezone).bind(hot).bind(now).execute(&mut *tx).await?;
   tx.commit().await?;Ok(watermark)
  }.await;
        source.close().await;
        serial.rollback().await?;
        result
    }
    pub async fn needs_refresh(&self, db: &TenantDb, now: DateTime<Utc>) -> Result<bool, AppError> {
        db.ensure_current_owner()?;
        let (sequence,zone):(i64,String)=sqlx::query_as("SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='outbox_events'),0),timezone FROM tenant_runtime WHERE singleton=1").fetch_one(&db.background_read_pool()?).await?;
        let current:Option<(i64,i64,String,DateTime<Utc>)>=sqlx::query_as("SELECT ownership_generation,source_watermark,timezone,next_due FROM hot_tenant_state WHERE tenant_id=$1").bind(db.tenant_id()).fetch_optional(&self.ledger.pool).await?;
        Ok(current.is_none_or(|(generation, last, stored, due)| {
            generation != db.ownership_generation()
                || sequence != last
                || zone != stored
                || now >= due
        }))
    }
}
pub struct Consumer {
    pub writer: Arc<Writer>,
    pub db: Arc<TenantDb>,
    consumer: async_nats::jetstream::consumer::PullConsumer,
}
impl Consumer {
    pub async fn connect(
        context: &async_nats::jetstream::Context,
        writer: Arc<Writer>,
        db: Arc<TenantDb>,
    ) -> Result<Self, AppError> {
        db.ensure_current_owner()?;
        let stream = context
            .get_stream(TENANT_EVENT_STREAM)
            .await
            .map_err(fail)?;
        let name = format!("hot_{}", db.tenant_id().simple());
        let subject = format!("arena.tenant.{}.events.v1", db.tenant_id());
        let mut consumer = stream
            .get_or_create_consumer(
                &name,
                async_nats::jetstream::consumer::pull::Config {
                    durable_name: Some(name.clone()),
                    filter_subject: subject.clone(),
                    deliver_policy: async_nats::jetstream::consumer::DeliverPolicy::All,
                    ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
                    ack_wait: Duration::from_secs(600),
                    max_ack_pending: 500,
                    max_waiting: 1,
                    max_batch: 500,
                    ..Default::default()
                },
            )
            .await
            .map_err(fail)?;
        let info = consumer.info().await.map_err(fail)?;
        if info.config.filter_subject != subject
            || info.config.ack_policy != async_nats::jetstream::consumer::AckPolicy::Explicit
            || info.config.deliver_policy != async_nats::jetstream::consumer::DeliverPolicy::All
            || info.config.max_ack_pending > 500
        {
            return Err(fail("Incompatible existing hot consumer"));
        }
        Ok(Self {
            writer,
            db,
            consumer,
        })
    }
    pub async fn poll(&mut self) -> Result<usize, AppError> {
        static BUFFERS: std::sync::LazyLock<tokio::sync::Semaphore> =
            std::sync::LazyLock::new(|| tokio::sync::Semaphore::new(4));
        let _buffer = BUFFERS.acquire().await.map_err(fail)?;
        let mut messages = self
            .consumer
            .batch()
            .max_messages(500)
            .max_bytes(8 * 1024 * 1024)
            .expires(Duration::from_secs(1))
            .messages()
            .await
            .map_err(fail)?;
        let mut buffered = vec![];
        while let Some(message) = messages.try_next().await.map_err(fail)? {
            if message.subject.as_str() != format!("arena.tenant.{}.events.v1", self.db.tenant_id())
            {
                return Err(fail("Foreign hot subject"));
            }
            let event: TenantEvent = serde_json::from_slice(&message.payload).map_err(fail)?;
            if event.tenant_id != self.db.tenant_id() || event.sequence < 1 {
                return Err(fail("Foreign or invalid hot event"));
            }
            buffered.push((message, event.sequence));
        }
        let now = Utc::now();
        let watermark = if self.writer.needs_refresh(&self.db, now).await? {
            self.writer.refresh(self.db.clone(), now).await?
        } else {
            sqlx::query_scalar("SELECT source_watermark FROM hot_tenant_state WHERE tenant_id=$1")
                .bind(self.db.tenant_id())
                .fetch_one(&self.writer.ledger.pool)
                .await?
        };
        self.db.ensure_current_owner()?;
        for (message, sequence) in &buffered {
            if *sequence > watermark {
                return Err(fail("Broker event is ahead of verified SQLite copy"));
            }
            message.ack().await.map_err(fail)?;
        }
        Ok(buffered.len())
    }
}
pub fn spawn(manager: Arc<TenantDbManager>, writer: Arc<Writer>, url: String) {
    tokio::spawn(async move {
        let mut workers =
            std::collections::HashMap::<Uuid, (i64, tokio::task::JoinHandle<()>)>::new();
        let mut tick = tokio::time::interval(Duration::from_secs(5));
        loop {
            tick.tick().await;
            let handles = manager.open_handles().await;
            let active = handles
                .iter()
                .map(|d| (d.tenant_id(), d.ownership_generation()))
                .collect::<std::collections::HashMap<_, _>>();
            workers.retain(|id, (generation, task)| {
                if active.get(id) != Some(generation) || task.is_finished() {
                    task.abort();
                    false
                } else {
                    true
                }
            });
            for db in handles {
                if workers.contains_key(&db.tenant_id()) {
                    continue;
                }
                let copy = writer.clone();
                let url = url.clone();
                let id = db.tenant_id();
                let generation = db.ownership_generation();
                let task = tokio::spawn(async move {
                    let result: Result<(), AppError> = async {
                        let client = async_nats::connect(url).await.map_err(fail)?;
                        let context = async_nats::jetstream::new(client);
                        let mut consumer = Consumer::connect(&context, copy.clone(), db).await?;
                        loop {
                            consumer.poll().await?;
                            tokio::time::sleep(Duration::from_secs(30)).await;
                        }
                    }
                    .await;
                    if let Err(error) = result {
                        copy.metrics.historical_finished("hot_consumer", false, 0);
                        tracing::warn!(%id,%error,"Hot copy remains retryable");
                    }
                });
                workers.insert(id, (generation, task));
            }
        }
    });
}

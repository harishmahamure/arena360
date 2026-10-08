//! Lossless archive restore with validated encrypted staging and replicated checkpoints.
use super::{
    archive,
    objects::{self, Object},
    policy, raw,
};
use crate::{
    error::AppError,
    metrics::Metrics,
    replication::{crypto::TenantKeys, ledger::PostgresLedger, snapshot},
    tenancy::{write_outbox_event_on_connection, NewOutboxEvent, TenantDb},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Row, SqliteConnection};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;
fn fail(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("Archive backfill: {e}"))
}
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Job {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub source_archive: Uuid,
    pub target_schema_version: i64,
    pub state: String,
    pub checkpoint: serde_json::Value,
    pub rows_processed: i64,
    pub staging_objects: serde_json::Value,
    pub validation_checksum: Option<String>,
    pub source_objects: serde_json::Value,
    pub expected_rows: Option<i64>,
    pub batch_rows: i32,
    pub oltp_p99_target_milliseconds: i64,
    pub last_error: Option<String>,
    pub retry_after: DateTime<Utc>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Input {
    revision: i64,
    object: Object,
}
fn checksum<T: Serialize>(value: &T) -> Result<String, AppError> {
    Ok(raw::row_hash(&serde_json::to_string(value).map_err(fail)?))
}
pub async fn get(pool: &PgPool, id: Uuid) -> Result<Job, AppError> {
    sqlx::query_as("SELECT * FROM historical_backfills WHERE id=$1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Backfill job not found".into()))
}
pub async fn enqueue(pool: &PgPool, source: Uuid, p99: i64, batch: i32) -> Result<Job, AppError> {
    if !(1..=60000).contains(&p99) || !(1..=1000).contains(&batch) {
        return Err(AppError::BadRequest(
            "Use p99 budget 1..60000 ms and batch 1..1000 rows".into(),
        ));
    }
    let source = archive::get(pool, source).await?;
    let mut tx = pool.begin().await?;
    let valid:bool=sqlx::query_scalar("SELECT state='ACTIVE' AND owner_cell IS NOT NULL AND schema_version=$2 AND EXISTS(SELECT 1 FROM tenant_leases l WHERE l.tenant_id=tenants.id AND l.owner_cell=tenants.owner_cell AND l.ownership_generation=tenants.ownership_generation AND l.expires_at>clock_timestamp()+INTERVAL '30 seconds') FROM tenants WHERE id=$1 FOR UPDATE").bind(source.tenant_id).bind(crate::tenancy::target_schema_version()).fetch_one(&mut *tx).await?;
    if !valid {
        return Err(AppError::Conflict(
            "Backfill requires an ACTIVE tenant on the current schema with a fresh lease".into(),
        ));
    }
    let busy:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM archive_manifests WHERE tenant_id=$1 AND period_start=$2 AND state<>'COMPLETE' AND superseded_at IS NULL) OR EXISTS(SELECT 1 FROM historical_backfills WHERE tenant_id=$1 AND state<>'COMPLETE')").bind(source.tenant_id).bind(source.period_start).fetch_one(&mut *tx).await?;
    if busy {
        return Err(AppError::Conflict(
            "Finish the tenant's archive and backfill jobs first".into(),
        ));
    }
    let revisions:Vec<archive::Job>=sqlx::query_as("SELECT * FROM archive_manifests WHERE tenant_id=$1 AND period_start=$2 AND state IN ('VERIFIED','PURGING','COMPLETE') AND revision<=$3 ORDER BY revision").bind(source.tenant_id).bind(source.period_start).bind(source.revision).fetch_all(&mut *tx).await?;
    if !revisions.iter().any(|r| r.id == source.id) {
        return Err(AppError::Conflict("Source archive is not verified".into()));
    }
    let mut inputs = vec![];
    for revision in revisions {
        let objects: Vec<Object> = serde_json::from_value(revision.objects).map_err(fail)?;
        if revision.checksum_sha256.as_deref() != Some(&checksum(&objects)?) {
            return Err(fail("Source manifest checksum differs"));
        }
        for object in objects {
            policy::table(&object.table)?;
            inputs.push(Input {
                revision: revision.revision,
                object,
            });
        }
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO historical_backfills(id,tenant_id,source_archive,target_schema_version,source_objects,batch_rows,oltp_p99_target_milliseconds) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(id).bind(source.tenant_id).bind(source.id).bind(crate::tenancy::target_schema_version()).bind(serde_json::to_value(inputs).map_err(fail)?).bind(batch).bind(p99).execute(&mut *tx).await?;
    tx.commit().await?;
    get(pool, id).await
}
pub struct Worker {
    pub ledger: Arc<PostgresLedger>,
    pub store: Arc<dyn object_store::ObjectStore>,
    pub keys: TenantKeys,
    pub metrics: Arc<Metrics>,
}
fn directory(db: &TenantDb, id: Uuid) -> Result<PathBuf, AppError> {
    let root = db
        .path()
        .parent()
        .ok_or_else(|| fail("Tenant directory missing"))?
        .join("historical/backfill")
        .join(id.to_string());
    std::fs::create_dir_all(&root).map_err(fail)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).map_err(fail)?;
    }
    Ok(root)
}
async fn open(path: &Path) -> Result<sqlx::SqlitePool, AppError> {
    Ok(sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await?)
}
async fn phase(pool: &PgPool, id: Uuid, next: &str) -> Result<(), AppError> {
    sqlx::query("UPDATE historical_backfills SET state=$2,started_at=COALESCE(started_at,clock_timestamp()),last_error=NULL WHERE id=$1").bind(id).bind(next).execute(pool).await?;
    Ok(())
}
/// Additive nullable/defaulted target fields are filled by SQLite. Removed or changed fields fail closed.
async fn insert(
    connection: &mut SqliteConnection,
    table: &str,
    source: &[raw::Column],
    payload: &str,
) -> Result<(), AppError> {
    let info = sqlx::query(&format!("PRAGMA table_info({})", raw::identifier(table)?))
        .fetch_all(&mut *connection)
        .await?;
    for col in source {
        let target = info
            .iter()
            .find(|r| r.get::<String, _>("name") == col.name)
            .ok_or_else(|| {
                AppError::Conflict(format!("No conversion for removed {table}.{}", col.name))
            })?;
        if target.get::<String, _>("type") != col.kind {
            return Err(AppError::Conflict(format!(
                "No conversion for changed {table}.{}",
                col.name
            )));
        }
    }
    let value: serde_json::Value = serde_json::from_str(payload).map_err(fail)?;
    let map = value
        .as_object()
        .ok_or_else(|| fail("Row payload must be an object"))?;
    if map.len() != source.len() {
        return Err(fail("Row payload does not match source columns"));
    }
    let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(format!(
        "INSERT INTO {} (",
        raw::identifier(table)?
    ));
    query.push(
        source
            .iter()
            .map(|c| raw::identifier(&c.name))
            .collect::<Result<Vec<_>, _>>()?
            .join(","),
    );
    query.push(") VALUES (");
    let mut values = query.separated(",");
    for col in source {
        let v = map
            .get(&col.name)
            .ok_or_else(|| fail("Missing archived column"))?;
        match (col.kind.as_str(), v) {
            (_, serde_json::Value::Null) => {
                values.push_bind(Option::<String>::None);
            }
            ("INTEGER", v) => {
                values.push_bind(v.as_i64().ok_or_else(|| fail("Invalid integer value"))?);
            }
            ("TEXT", v) => {
                values.push_bind(
                    v.as_str()
                        .ok_or_else(|| fail("Invalid text value"))?
                        .to_owned(),
                );
            }
            _ => return Err(fail("Unsupported archived column")),
        };
    }
    query.push(")");
    let id = map
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| fail("Invalid row id"))?;
    let current = sqlx::query(&raw::row_select(table, source, "r.id=?")?)
        .bind(id)
        .fetch_optional(&mut *connection)
        .await?;
    if let Some(current) = current {
        let current: serde_json::Value =
            serde_json::from_str(&current.get::<String, _>("payload")).map_err(fail)?;
        if current != value {
            return Err(AppError::Conflict(format!(
                "Live {table} row {id} differs from archive; no overwrite permitted"
            )));
        }
        return Ok(());
    }
    query.build().execute(connection).await?;
    Ok(())
}
impl Worker {
    async fn admitted(&self, db: &TenantDb, job: &Job) -> Result<(), AppError> {
        db.ensure_current_owner()?;
        if job.tenant_id != db.tenant_id()
            || job.target_schema_version != crate::tenancy::target_schema_version()
        {
            return Err(AppError::Conflict(
                "Backfill target identity/schema changed".into(),
            ));
        }
        let mut tx = self.ledger.pool.begin().await?;
        self.ledger.lock_owner(db, &mut tx).await?;
        tx.rollback().await?;
        Ok(())
    }
    async fn stage(&self, db: Arc<TenantDb>, job: Job) -> Result<(), AppError> {
        self.admitted(&db, &job).await?;
        let _permit = match db.background_jobs() {
            Some(j) => Some(j.acquire(crate::background::Priority::Maintenance).await?),
            None => None,
        };
        let root = directory(&db, job.id)?;
        let inputs: Vec<Input> =
            serde_json::from_value(job.source_objects.clone()).map_err(fail)?;
        if inputs.is_empty() {
            return Err(fail("Backfill source objects missing"));
        }
        let key = self.keys.read(job.tenant_id)?;
        let stages = ["PLANNED", "DOWNLOADING", "TRANSFORMING", "VALIDATING"];
        let mut current = stages
            .iter()
            .position(|s| *s == job.state)
            .ok_or_else(|| fail("Unexpected staging phase"))?;
        if current == 0 {
            phase(&self.ledger.pool, job.id, "DOWNLOADING").await?;
            current = 1;
        }
        let index_path = root.join("index.sqlite");
        let _ = std::fs::remove_file(&index_path);
        let index = open(&index_path).await?;
        sqlx::query("CREATE TABLE selected(table_name TEXT NOT NULL,id TEXT NOT NULL,payload TEXT NOT NULL,checksum TEXT NOT NULL,columns TEXT NOT NULL,revision INTEGER NOT NULL,PRIMARY KEY(table_name,id)) STRICT").execute(&index).await?;
        for (n, input) in inputs.iter().enumerate() {
            let path = root.join(format!("source-{n}.parquet"));
            objects::download(self.store.as_ref(), &key, &input.object, &path).await?;
            let mut after = None;
            let mut count = 0;
            loop {
                let rows = raw::batch(path.clone(), after.clone(), 1000).await?;
                if rows.is_empty() {
                    break;
                }
                after = Some(rows.last().unwrap().0.clone());
                let mut tx = index.begin().await?;
                for (id, payload, hash) in rows {
                    if raw::row_hash(&payload) != hash {
                        return Err(fail("Source row checksum differs"));
                    }
                    let v: serde_json::Value = serde_json::from_str(&payload).map_err(fail)?;
                    if v["id"] != id {
                        return Err(fail("Source row id differs"));
                    }
                    sqlx::query("INSERT INTO selected VALUES(?,?,?,?,?,?) ON CONFLICT(table_name,id) DO UPDATE SET payload=excluded.payload,checksum=excluded.checksum,columns=excluded.columns,revision=excluded.revision WHERE excluded.revision>selected.revision").bind(&input.object.table).bind(id).bind(payload).bind(hash).bind(serde_json::to_string(&input.object.columns).map_err(fail)?).bind(input.revision).execute(&mut *tx).await?;
                    count += 1;
                }
                tx.commit().await?;
            }
            if count != input.object.rows {
                return Err(fail("Source row count differs"));
            }
        }
        if current == 1 {
            phase(&self.ledger.pool, job.id, "TRANSFORMING").await?;
            current = 2;
        }
        let image = root.join("validation.sqlite");
        let _ = std::fs::remove_file(&image);
        sqlx::query("VACUUM INTO ?")
            .bind(image.to_str().ok_or_else(|| fail("Invalid staging path"))?)
            .execute(&db.background_read_pool()?)
            .await?;
        let stage = open(&image).await?;
        let order = policy::deletion_order(&policy::references(&stage).await?)?;
        let mut outputs = vec![];
        let mut total = 0;
        for spec in order.into_iter().rev() {
            sqlx::query("CREATE TABLE IF NOT EXISTS historical_selected_ids(table_name TEXT NOT NULL,id TEXT NOT NULL,PRIMARY KEY(table_name,id)) STRICT").execute(&stage).await?;
            let mut after = String::new();
            loop {
                let rows=sqlx::query("SELECT id,payload,checksum,columns FROM selected WHERE table_name=? AND id>? ORDER BY id LIMIT 1000").bind(spec.name).bind(&after).fetch_all(&index).await?;
                if rows.is_empty() {
                    break;
                }
                let mut tx = stage.begin().await?;
                for row in rows {
                    let id: String = row.get("id");
                    let payload: String = row.get("payload");
                    let cols: Vec<raw::Column> =
                        serde_json::from_str(&row.get::<String, _>("columns")).map_err(fail)?;
                    insert(&mut tx, spec.name, &cols, &payload).await?;
                    sqlx::query("INSERT INTO historical_selected_ids VALUES(?,?)")
                        .bind(spec.name)
                        .bind(&id)
                        .execute(&mut *tx)
                        .await?;
                    after = id;
                }
                tx.commit().await?;
            }
            let columns = raw::columns(&stage, spec.name).await?;
            let predicate = format!(
                "r.id IN (SELECT id FROM historical_selected_ids WHERE table_name={})",
                raw::literal(spec.name)
            );
            let (path, rows, hash) =
                raw::parquet(&stage, spec.name, &columns, &predicate, &root).await?;
            let mut object = objects::upload(
                self.store.as_ref(),
                &key,
                format!(
                    "tenants/{}/backfills/{}/{}-{}.parquet",
                    job.tenant_id,
                    job.id,
                    spec.name,
                    Uuid::new_v4()
                ),
                spec.name.into(),
                rows,
                hash,
                &path,
            )
            .await?;
            object.columns = columns;
            total += rows;
            outputs.push(object);
        }
        if current == 2 {
            phase(&self.ledger.pool, job.id, "VALIDATING").await?;
        }
        if !sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&stage)
            .await?
            .is_empty()
        {
            return Err(fail("Staging foreign key validation failed"));
        }
        let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&stage)
            .await?;
        if integrity != "ok" {
            return Err(fail("Staging integrity check failed"));
        }
        stage.close().await;
        index.close().await;
        self.admitted(&db, &job).await?;
        let mut tx = self.ledger.pool.begin().await?;
        self.ledger.lock_owner(&db, &mut tx).await?;
        sqlx::query("UPDATE historical_backfills SET state='BACKFILLING',staging_objects=$2,validation_checksum=$3,expected_rows=$4,last_error=NULL WHERE id=$1 AND state='VALIDATING'").bind(job.id).bind(serde_json::to_value(&outputs).map_err(fail)?).bind(checksum(&outputs)?).bind(total).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}
fn aggregate(table: &str, payload: &serde_json::Value) -> Result<Option<(String, Uuid)>, AppError> {
    let (kind, key) = match table {
        "transactions" => ("transaction", "id"),
        "transaction_products" => ("transaction", "transaction_id"),
        "usage_sessions" => ("session", "id"),
        "shifts" => ("shift", "id"),
        "credit_settlements" => ("credit_settlement", "id"),
        "credit_settlement_items" => ("credit_settlement", "settlement_id"),
        "expenses" => ("expense", "id"),
        "cash_registers" => ("cash_register", "id"),
        "cash_register_entries" => ("cash_register", "cash_register_id"),
        "cash_deposits" => ("cash_deposit", "id"),
        "purchase_orders" => ("purchase_order", "id"),
        "purchase_order_lines" => ("purchase_order", "purchase_order_id"),
        "stock_transfer_requests" => ("stock_transfer", "id"),
        "stock_transfer_lines" => ("stock_transfer", "transfer_request_id"),
        "stock_receipts" => ("stock_receipt", "id"),
        "stock_receipt_lines" => ("stock_receipt", "receipt_id"),
        "stock_waste_events" => ("stock_waste", "id"),
        "stock_waste_lines" => ("stock_waste", "waste_event_id"),
        "stock_movements" => ("stock_movement", "id"),
        _ => return Ok(None),
    };
    Ok(Some((
        kind.into(),
        payload[key]
            .as_str()
            .ok_or_else(|| fail("Analytics aggregate key missing"))?
            .parse()
            .map_err(fail)?,
    )))
}
impl Worker {
    async fn apply(&self, db: Arc<TenantDb>, job: Job) -> Result<bool, AppError> {
        self.admitted(&db, &job).await?;
        if db.foreground_write_p99_micros() > job.oltp_p99_target_milliseconds as u64 * 1000 {
            sqlx::query("UPDATE historical_backfills SET batch_rows=GREATEST(1,batch_rows/2),retry_after=clock_timestamp()+INTERVAL '30 seconds',last_error='Foreground write p99 exceeds backfill budget' WHERE id=$1").bind(job.id).execute(&self.ledger.pool).await?;
            return Ok(false);
        }
        let objects: Vec<Object> =
            serde_json::from_value(job.staging_objects.clone()).map_err(fail)?;
        let evidence = checksum(&objects)?;
        if job.validation_checksum.as_deref() != Some(&evidence)
            || objects.len() != policy::TABLES.len()
        {
            return Err(fail("Validated staging evidence differs"));
        }
        let expected = job
            .expected_rows
            .ok_or_else(|| fail("Validated row count missing"))?;
        if objects.iter().map(|o| o.rows).sum::<i64>() != expected {
            return Err(fail("Staging row total differs"));
        }
        let root = directory(&db, job.id)?;
        let key = self.keys.read(job.tenant_id)?;
        let source = db.background_read_pool()?;
        let schema: i64 = sqlx::query_scalar("PRAGMA schema_version")
            .fetch_one(&source)
            .await?;
        let local:Option<(String,String,i64,i64)>=sqlx::query_as("SELECT validation_checksum,checkpoint,rows_processed,completed FROM archive_backfill_checkpoints WHERE backfill_id=?").bind(job.id.to_string()).fetch_optional(&source).await?;
        if local.as_ref().is_some_and(|(h, _, _, _)| h != &evidence) {
            return Err(fail("Local checkpoint evidence differs"));
        }
        let mut progress: BTreeMap<String, String> = local
            .as_ref()
            .map(|(_, p, _, _)| serde_json::from_str(p).map_err(fail))
            .transpose()?
            .unwrap_or_default();
        let processed = local.as_ref().map(|(_, _, n, _)| *n).unwrap_or(0);
        if processed < job.rows_processed {
            return Err(fail(
                "Local backfill checkpoint regressed; recover its replicated WAL before retrying",
            ));
        }
        let order = policy::deletion_order(&policy::references(&source).await?)?;
        for spec in order.into_iter().rev() {
            let object = objects
                .iter()
                .find(|o| o.table == spec.name)
                .ok_or_else(|| fail("Staging omits a fact table"))?;
            if object.columns != raw::columns(&source, spec.name).await? {
                return Err(AppError::Conflict(
                    "Target schema changed after staging validation".into(),
                ));
            }
            let path = root.join(format!("validated-{}.parquet", spec.name));
            if !path.exists() || snapshot::hash_file(&path)? != object.plaintext_checksum {
                objects::download(self.store.as_ref(), &key, object, &path).await?;
            }
            let rows = raw::batch(
                path,
                progress.get(spec.name).cloned(),
                job.batch_rows as usize,
            )
            .await?;
            if rows.is_empty() {
                continue;
            }
            if job.state == "VERIFYING" {
                return Err(fail("Verifying backfill has incomplete checkpoints"));
            }
            let _permit = match db.background_jobs() {
                Some(j) => Some(j.acquire(crate::background::Priority::Maintenance).await?),
                None => None,
            };
            let count = rows.len() as i64;
            let total = processed + count;
            if total > expected {
                return Err(fail("Backfill exceeds validated rows"));
            }
            progress.insert(spec.name.into(), rows.last().unwrap().0.clone());
            let progress_json = serde_json::to_string(&progress).map_err(fail)?;
            let ledger = self.ledger.clone();
            let owned = db.clone();
            let id = job.id;
            let hash = evidence.clone();
            let checkpoint = progress_json.clone();
            let table = spec.name.to_string();
            let columns = object.columns.clone();
            let mut tx=db.with_background_immediate_writer(move|c|Box::pin(async move{
 if sqlx::query_scalar::<_,i64>("PRAGMA schema_version").fetch_one(&mut *c).await?!=schema{return Err(AppError::Conflict("Target schema changed during admission".into()));}
 let mut tx=ledger.pool.begin().await?;ledger.lock_owner(&owned,&mut tx).await?;let admitted:bool=sqlx::query_scalar("SELECT state='BACKFILLING' AND validation_checksum=$2 FROM historical_backfills WHERE id=$1 FOR UPDATE").bind(id).bind(&hash).fetch_one(&mut *tx).await?;if !admitted{return Err(AppError::Conflict("Backfill changed during admission".into()));}
 let previous:Option<(String,i64)>=sqlx::query_as("SELECT validation_checksum,rows_processed FROM archive_backfill_checkpoints WHERE backfill_id=?").bind(id.to_string()).fetch_optional(&mut *c).await?;if previous.as_ref().is_some_and(|(h,n)|h!=&hash||*n!=processed)||(previous.is_none()&&processed!=0){return Err(AppError::Conflict("Backfill checkpoint changed".into()));}
 let mut events=BTreeMap::new();for(row_id,payload,sum)in rows{if raw::row_hash(&payload)!=sum{return Err(fail("Staged row checksum differs"));}let value:serde_json::Value=serde_json::from_str(&payload).map_err(fail)?;if value["id"]!=row_id{return Err(fail("Staged row id differs"));}insert(c,&table,&columns,&payload).await?;if table=="transaction_product_options"{let parent=value["transaction_product_id"].as_str().ok_or_else(||fail("Transaction option parent missing"))?;let root:String=sqlx::query_scalar("SELECT transaction_id FROM transaction_products WHERE id=?").bind(parent).fetch_one(&mut *c).await?;events.insert(("transaction".into(),root.parse::<Uuid>().map_err(fail)?),());}else if let Some((kind,aggregate_id))=aggregate(&table,&value)?{events.insert((kind,aggregate_id),());}}
 for((kind,aggregate_id),_)in events{write_outbox_event_on_connection(c,NewOutboxEvent{location_id:None,aggregate_type:kind,aggregate_id,event_type:"historical.backfilled".into(),schema_version:1,deleted:false,payload:serde_json::json!({"backfillId":id})}).await?;}
 sqlx::query("INSERT INTO archive_backfill_checkpoints(backfill_id,validation_checksum,checkpoint,rows_processed,updated_at) VALUES(?,?,?,?,?) ON CONFLICT(backfill_id) DO UPDATE SET checkpoint=excluded.checkpoint,rows_processed=excluded.rows_processed,updated_at=excluded.updated_at").bind(id.to_string()).bind(&hash).bind(checkpoint).bind(total).bind(crate::time::format_sqlite_timestamp(&Utc::now()).map_err(fail)?).execute(c).await?;Ok(tx)
 })).await?;
            sqlx::query("UPDATE historical_backfills SET rows_processed=GREATEST(rows_processed,$2),checkpoint=CASE WHEN rows_processed<=$2 THEN $3 ELSE checkpoint END,last_error=NULL WHERE id=$1").bind(job.id).bind(total).bind(serde_json::from_str::<serde_json::Value>(&progress_json).map_err(fail)?).execute(&mut *tx).await?;
            tx.commit().await?;
            return Ok(false);
        }
        if processed != expected {
            return Err(fail("Backfill checkpoint does not cover validated rows"));
        }
        if job.state == "BACKFILLING" {
            sqlx::query("UPDATE historical_backfills SET state='VERIFYING',rows_processed=GREATEST(rows_processed,$2),checkpoint=$3 WHERE id=$1 AND state='BACKFILLING'").bind(job.id).bind(processed).bind(serde_json::to_value(&progress).map_err(fail)?).execute(&self.ledger.pool).await?;
            return Ok(false);
        }
        // Detect concurrent commits with one retained read connection; the final writer admission fences the last check.
        let mut verification = source.acquire().await?;
        let data_version: i64 = sqlx::query_scalar("PRAGMA data_version")
            .fetch_one(&mut *verification)
            .await?;
        // Compare every restored row again in bounded reader batches before declaring completion.
        for object in &objects {
            let path = root.join(format!("validated-{}.parquet", object.table));
            let mut after = None;
            loop {
                let rows = raw::batch(path.clone(), after.clone(), 1000).await?;
                if rows.is_empty() {
                    break;
                }
                after = Some(rows.last().unwrap().0.clone());
                let select = raw::row_select(&object.table, &object.columns, "r.id=?")?;
                for (id, payload, hash) in rows {
                    if raw::row_hash(&payload) != hash {
                        return Err(fail("Verification staging row checksum differs"));
                    }
                    let live: Option<String> = sqlx::query(&select)
                        .bind(id)
                        .fetch_optional(&mut *verification)
                        .await?
                        .map(|r| r.get("payload"));
                    if live.as_deref() != Some(&payload) {
                        return Err(AppError::Conflict(
                            "Restored rows changed before completion".into(),
                        ));
                    }
                }
            }
        }
        let ledger = self.ledger.clone();
        let owned = db.clone();
        let id = job.id;
        let hash = evidence.clone();
        let mut tx=db.with_background_immediate_writer(move|c|Box::pin(async move{if sqlx::query_scalar::<_,i64>("PRAGMA data_version").fetch_one(&mut *verification).await?!=data_version{return Err(AppError::Conflict("Live writes occurred during backfill verification; retry".into()));}let mut tx=ledger.pool.begin().await?;ledger.lock_owner(&owned,&mut tx).await?;let valid:bool=sqlx::query_scalar("SELECT state='VERIFYING' AND validation_checksum=$2 AND rows_processed=$3 FROM historical_backfills WHERE id=$1 FOR UPDATE").bind(id).bind(&hash).bind(expected).fetch_one(&mut *tx).await?;if !valid{return Err(AppError::Conflict("Backfill completion was fenced".into()));}sqlx::query("INSERT INTO archive_backfill_checkpoints(backfill_id,validation_checksum,rows_processed,completed,updated_at) VALUES(?,?,?,1,?) ON CONFLICT(backfill_id) DO UPDATE SET completed=1,updated_at=excluded.updated_at").bind(id.to_string()).bind(hash).bind(expected).bind(crate::time::format_sqlite_timestamp(&Utc::now()).map_err(fail)?).execute(c).await?;Ok(tx)})).await?;
        sqlx::query("UPDATE historical_backfills SET state='COMPLETE',completed_at=clock_timestamp(),last_error=NULL WHERE id=$1").bind(job.id).execute(&mut *tx).await?;
        tx.commit().await?;
        std::fs::remove_dir_all(root).map_err(fail)?;
        Ok(true)
    }
    pub async fn advance(&self, db: Arc<TenantDb>, id: Uuid) -> Result<bool, AppError> {
        let started = std::time::Instant::now();
        let result = async {
            let mut serial = self.ledger.pool.begin().await?;
            let locked: bool =
                sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
                    .bind(format!("historical-backfill:{id}"))
                    .fetch_one(&mut *serial)
                    .await?;
            if !locked {
                return Ok(false);
            }
            let job = get(&self.ledger.pool, id).await?;
            let done = match job.state.as_str() {
                "PLANNED" | "DOWNLOADING" | "TRANSFORMING" | "VALIDATING" => {
                    self.stage(db, job).await?;
                    false
                }
                "BACKFILLING" | "VERIFYING" => self.apply(db, job).await?,
                "COMPLETE" => true,
                _ => return Err(fail("Unexpected backfill phase")),
            };
            serial.rollback().await?;
            Ok(done)
        }
        .await;
        self.metrics.historical_finished(
            "archive_backfill",
            result.is_ok(),
            started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        );
        result
    }
    pub fn spawn(self: Arc<Self>, manager: Arc<crate::tenancy::TenantDbManager>) {
        tokio::spawn(async move {
            let mut ticks = tokio::time::interval(Duration::from_secs(1));
            ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticks.tick().await;
                let jobs=sqlx::query_as::<_,Job>("SELECT b.* FROM historical_backfills b JOIN tenants t ON t.id=b.tenant_id WHERE t.owner_cell=$1 AND t.state='ACTIVE' AND b.state<>'COMPLETE' AND b.retry_after<=clock_timestamp() ORDER BY b.created_at LIMIT 8").bind(self.ledger.cell_id).fetch_all(&self.ledger.pool).await;
                let jobs = match jobs {
                    Ok(j) => j,
                    Err(error) => {
                        tracing::warn!(%error,"Backfill queue unavailable");
                        continue;
                    }
                };
                for job in jobs {
                    let result = async {
                        self.advance(manager.open(job.tenant_id).await?, job.id)
                            .await
                    }
                    .await;
                    if let Err(error) = result {
                        tracing::warn!(backfill=%job.id,%error,"Backfill remains retryable");
                        let _=sqlx::query("UPDATE historical_backfills SET last_error=$2,retry_after=clock_timestamp()+INTERVAL '30 seconds' WHERE id=$1 AND state<>'COMPLETE'").bind(job.id).bind(error.to_string()).execute(&self.ledger.pool).await;
                    }
                }
            }
        });
    }
}

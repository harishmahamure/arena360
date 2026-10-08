//! Verified monthly archives and resumable, lease-fenced small purge transactions.
use super::{
    hot,
    objects::{self, Object},
    policy, raw,
};
use crate::{
    analytics::calendar,
    error::AppError,
    metrics::Metrics,
    replication::{crypto::TenantKeys, ledger::PostgresLedger, wal},
    tenancy::TenantDb,
};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use chrono_tz::Tz;
use sqlx::{FromRow, PgPool, Row};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;
fn fail(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("Archive: {e}"))
}
#[derive(Debug, Clone, FromRow, serde::Serialize)]
pub struct Job {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub source_cell: Uuid,
    pub ownership_generation: i64,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub timezone: String,
    pub schema_version: i64,
    pub source_watermark: i64,
    pub state: String,
    pub row_count: Option<i64>,
    pub checksum_sha256: Option<String>,
    pub objects: serde_json::Value,
    pub rows_purged: i64,
    pub revision: i64,
    pub superseded_at: Option<DateTime<Utc>>,
    pub oltp_p99_target_milliseconds: i64,
    pub batch_rows: i32,
    pub last_error: Option<String>,
    pub source_cutoff_at: Option<DateTime<Utc>>,
    pub hot_cleaned_at: Option<DateTime<Utc>>,
}
pub async fn get(pool: &PgPool, id: Uuid) -> Result<Job, AppError> {
    sqlx::query_as("SELECT * FROM archive_manifests WHERE id=$1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Archive job not found".into()))
}
pub async fn enqueue(
    pool: &PgPool,
    tenant: Uuid,
    start: NaiveDate,
    p99: i64,
    batch: i32,
) -> Result<Job, AppError> {
    let mut tx = pool.begin().await?;
    let id = enqueue_locked(&mut tx, tenant, start, p99, batch).await?;
    tx.commit().await?;
    get(pool, id).await
}
async fn enqueue_locked(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant: Uuid,
    start: NaiveDate,
    p99: i64,
    batch: i32,
) -> Result<Uuid, AppError> {
    if start.day() != 1 || !(1..=60000).contains(&p99) || !(1..=1000).contains(&batch) {
        return Err(AppError::BadRequest(
            "Use a month boundary, p99 budget 1..60000 ms and batch 1..1000 rows".into(),
        ));
    }
    let row=sqlx::query("SELECT t.owner_cell,t.ownership_generation,t.state,t.timezone,t.schema_version,clock_timestamp() AS now FROM tenants t WHERE id=$1 FOR UPDATE").bind(tenant).fetch_optional(&mut **tx).await?.ok_or_else(||AppError::NotFound("Tenant not found".into()))?;
    let owner: Option<Uuid> = row.get(0);
    let generation: i64 = row.get(1);
    let timezone: String = row.get(3);
    let zone = timezone.parse::<Tz>().map_err(fail)?;
    let now: DateTime<Utc> = row.get(5);
    let end = hot::month(start, 1)?;
    if owner.is_none()
        || row.get::<String, _>(2) != "ACTIVE"
        || row.get::<i64, _>(4) != crate::tenancy::target_schema_version()
    {
        return Err(AppError::Conflict(
            "Archiving requires an ACTIVE tenant on the current schema".into(),
        ));
    }
    let cutoff = calendar::boundary(
        hot::month(now.with_timezone(&zone).date_naive(), -18)?,
        zone,
    )?;
    if calendar::boundary(end, zone)? > cutoff {
        return Err(AppError::BadRequest(
            "Archive only complete months older than the 18-month hot window".into(),
        ));
    }
    let fresh:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_leases WHERE tenant_id=$1 AND owner_cell=$2 AND ownership_generation=$3 AND expires_at>clock_timestamp()+INTERVAL '30 seconds')").bind(tenant).bind(owner).bind(generation).fetch_one(&mut **tx).await?;
    if !fresh {
        return Err(AppError::Forbidden(
            "Archive source lease is not fresh".into(),
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO archive_manifests(id,tenant_id,source_cell,ownership_generation,period_start,period_end,timezone,schema_version,oltp_p99_target_milliseconds,batch_rows,source_cutoff_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)").bind(id).bind(tenant).bind(owner).bind(generation).bind(start).bind(end).bind(timezone).bind(crate::tenancy::target_schema_version()).bind(p99).bind(batch).bind(cutoff).execute(&mut **tx).await?;
    Ok(id)
}
pub async fn replan(pool: &PgPool, id: Uuid) -> Result<Job, AppError> {
    let old = get(pool, id).await?;
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT id FROM tenants WHERE id=$1 FOR UPDATE")
        .bind(old.tenant_id)
        .fetch_one(&mut *tx)
        .await?;
    sqlx::query("UPDATE archive_manifests SET superseded_at=COALESCE(superseded_at,clock_timestamp()) WHERE id=$1").bind(id).execute(&mut *tx).await?;
    let next = enqueue_locked(
        &mut tx,
        old.tenant_id,
        old.period_start,
        old.oltp_p99_target_milliseconds,
        old.batch_rows,
    )
    .await?;
    tx.commit().await?;
    get(pool, next).await
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Capture {
    watermark: i64,
    objects: Vec<Object>,
}
fn save(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let temp = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    wal::durable_create(&temp, bytes)?;
    std::fs::rename(temp, path).map_err(fail)?;
    std::fs::File::open(path.parent().unwrap())
        .and_then(|f| f.sync_all())
        .map_err(fail)?;
    Ok(())
}
fn checksum(objects: &[Object]) -> Result<String, AppError> {
    Ok(raw::row_hash(
        &serde_json::to_string(objects).map_err(fail)?,
    ))
}
fn cutoff(job: &Job) -> Result<String, AppError> {
    crate::time::format_sqlite_timestamp(
        &job.source_cutoff_at
            .ok_or_else(|| AppError::Conflict("Archive lacks a frozen cutoff; replan".into()))?,
    )
    .map_err(fail)
}
fn directory(db: &TenantDb, id: Uuid) -> Result<PathBuf, AppError> {
    let path = db
        .path()
        .parent()
        .unwrap()
        .join("historical/archive")
        .join(id.to_string());
    std::fs::create_dir_all(&path).map_err(fail)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).map_err(fail)?;
    }
    Ok(path)
}
async fn open_snapshot(path: &Path) -> Result<sqlx::SqlitePool, AppError> {
    Ok(sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(path)
                .read_only(true),
        )
        .await?)
}
pub struct Worker {
    pub ledger: Arc<PostgresLedger>,
    pub store: Arc<dyn object_store::ObjectStore>,
    pub keys: TenantKeys,
    pub metrics: Arc<Metrics>,
}
impl Worker {
    pub async fn export(&self, db: Arc<TenantDb>, id: Uuid) -> Result<(), AppError> {
        let started = std::time::Instant::now();
        let result = self.export_inner(db, id).await;
        self.metrics.historical_finished(
            "archive_export",
            result.is_ok(),
            started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        );
        result
    }
    async fn export_inner(&self, db: Arc<TenantDb>, id: Uuid) -> Result<(), AppError> {
        db.ensure_current_owner()?;
        let job = get(&self.ledger.pool, id).await?;
        if job.tenant_id != db.tenant_id() || job.superseded_at.is_some() {
            return Err(AppError::Conflict(
                "Archive job no longer owns this dataset".into(),
            ));
        }
        if matches!(
            job.state.as_str(),
            "UPLOADED" | "VERIFIED" | "PURGING" | "COMPLETE"
        ) {
            return Ok(());
        }
        if job.source_cell != self.ledger.cell_id
            || job.ownership_generation != db.ownership_generation()
        {
            return Err(AppError::Conflict(
                "Unverified archive source moved; replan on its current owner".into(),
            ));
        }
        let key = self.keys.read(job.tenant_id)?;
        let root = directory(&db, id)?;
        let image = root.join("source.sqlite");
        let capture_path = root.join("capture.json");
        let mut tx = self.ledger.pool.begin().await?;
        self.ledger.lock_owner(&db, &mut tx).await?;
        sqlx::query("UPDATE archive_manifests SET state='EXPORTING',updated_at=clock_timestamp() WHERE id=$1 AND state='PLANNED' AND superseded_at IS NULL").bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        let capture_result=async{
   let snapshot_permit=match db.background_jobs(){Some(j)=>Some(j.acquire(crate::background::Priority::ArchiveExport).await?),None=>None};
   if !image.exists(){sqlx::query("VACUUM INTO ?").bind(image.to_str().ok_or_else(||fail("Invalid snapshot path"))?).execute(&db.background_read_pool()?).await?;}db.ensure_current_owner()?;
   drop(snapshot_permit);
   let source=open_snapshot(&image).await?;
   let result=async{
    let (watermark,zone):(i64,String)=sqlx::query_as("SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='outbox_events'),0),timezone FROM tenant_runtime WHERE singleton=1").fetch_one(&source).await?;let version:i64=sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success=1").fetch_one(&source).await?;
    if version!=job.schema_version||zone!=job.timezone{return Err(AppError::Conflict("Archive source schema or calendar changed; replan".into()));}
    let mut capture=if capture_path.exists(){serde_json::from_slice::<Capture>(&std::fs::read(&capture_path).map_err(fail)?).map_err(fail)?}else{Capture{watermark,objects:vec![]}};
    let zone=zone.parse::<Tz>().map_err(fail)?;let start=crate::time::format_sqlite_timestamp(&calendar::boundary(job.period_start,zone)?).map_err(fail)?;let end=crate::time::format_sqlite_timestamp(&calendar::boundary(job.period_end,zone)?).map_err(fail)?;
    for spec in policy::TABLES{
     if capture.objects.iter().any(|o|o.table==spec.name){continue;}
     let _permit=match db.background_jobs(){Some(j)=>Some(j.acquire(crate::background::Priority::ArchiveExport).await?),None=>None};
     let columns=raw::columns(&source,spec.name).await?;let predicate=policy::predicate(spec,&start,&end,&cutoff(&job)?)?;let (file,rows,source_checksum)=raw::parquet(&source,spec.name,&columns,&predicate,&root).await?;
     let object_key=format!("tenants/{}/archive/{}/{:02}/{}-{}-{}.parquet",job.tenant_id,job.period_start.year(),job.period_start.month(),job.id,spec.name,Uuid::new_v4());
     let mut object=objects::upload(self.store.as_ref(),&key,object_key,spec.name.into(),rows,source_checksum,&file).await?;object.columns=columns;capture.objects.push(object);save(&capture_path,&serde_json::to_vec(&capture).map_err(fail)?)?;
    }Ok::<_,AppError>(capture)
   }.await;source.close().await;result
  }.await?;
        let mut tx = self.ledger.pool.begin().await?;
        self.ledger.lock_owner(&db, &mut tx).await?;
        let current: String = sqlx::query_scalar(
            "SELECT state FROM archive_manifests WHERE id=$1 AND superseded_at IS NULL FOR UPDATE",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        if current == "PLANNED" {
            sqlx::query("UPDATE archive_manifests SET state='EXPORTING' WHERE id=$1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("UPDATE archive_manifests SET state='UPLOADED',source_watermark=$2,row_count=$3,checksum_sha256=$4,objects=$5,last_error=NULL,updated_at=clock_timestamp() WHERE id=$1 AND state='EXPORTING'").bind(id).bind(capture_result.watermark).bind(capture_result.objects.iter().map(|o|o.rows).sum::<i64>()).bind(checksum(&capture_result.objects)?).bind(serde_json::to_value(&capture_result.objects).map_err(fail)?).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn verify(&self, db: Arc<TenantDb>, id: Uuid) -> Result<(), AppError> {
        let started = std::time::Instant::now();
        let result = self.verify_inner(db, id).await;
        self.metrics.historical_finished(
            "archive_verify",
            result.is_ok(),
            started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        );
        result
    }
    async fn verify_inner(&self, db: Arc<TenantDb>, id: Uuid) -> Result<(), AppError> {
        let job = get(&self.ledger.pool, id).await?;
        if job.tenant_id != db.tenant_id() || job.superseded_at.is_some() {
            return Err(AppError::Conflict("Archive dataset changed".into()));
        }
        if job.state != "UPLOADED" {
            return Err(AppError::Conflict(
                "Verification requires an uploaded archive".into(),
            ));
        }
        let objects: Vec<Object> = serde_json::from_value(job.objects.clone()).map_err(fail)?;
        if job.checksum_sha256.as_deref() != Some(checksum(&objects)?.as_str()) {
            return Err(fail("Archive manifest checksum differs"));
        }
        let _permit = match db.background_jobs() {
            Some(j) => Some(
                j.acquire(crate::background::Priority::ArchiveExport)
                    .await?,
            ),
            None => None,
        };
        let key = self.keys.read(job.tenant_id)?;
        let root = directory(&db, id)?;
        let image = root.join(format!("verify-{}.sqlite", Uuid::new_v4()));
        sqlx::query("VACUUM INTO ?")
            .bind(
                image
                    .to_str()
                    .ok_or_else(|| fail("Invalid verification path"))?,
            )
            .execute(&db.background_read_pool()?)
            .await?;
        db.ensure_current_owner()?;
        let source = open_snapshot(&image).await?;
        let result=async{
   let zone=job.timezone.parse::<Tz>().map_err(fail)?;let start=crate::time::format_sqlite_timestamp(&calendar::boundary(job.period_start,zone)?).map_err(fail)?;let end=crate::time::format_sqlite_timestamp(&calendar::boundary(job.period_end,zone)?).map_err(fail)?;
   for object in &objects{
    let spec=policy::table(&object.table)?;if object.columns!=raw::columns(&source,spec.name).await?{return Err(AppError::Conflict("Archive source columns changed; replan".into()));}
    objects::download(self.store.as_ref(),&key,object,&root.join(format!("verified-{}.parquet",spec.name))).await?;
    let (rows,hash)=raw::fingerprint(&source,spec.name,&object.columns,&policy::predicate(spec,&start,&end,&cutoff(&job)?)?).await?;if rows!=object.rows||hash!=object.source_checksum{return Err(AppError::Conflict("Archive source rows changed; replan".into()));}
   }
   let mut tx=self.ledger.pool.begin().await?;self.ledger.lock_owner(&db,&mut tx).await?;
   let changed=sqlx::query("UPDATE archive_manifests SET state='VERIFIED',verified_at=clock_timestamp(),last_error=NULL,updated_at=clock_timestamp() WHERE id=$1 AND state='UPLOADED' AND superseded_at IS NULL AND checksum_sha256=$2").bind(id).bind(&job.checksum_sha256).execute(&mut *tx).await?.rows_affected();if changed!=1{return Err(AppError::Conflict("Archive changed during verification".into()));}tx.commit().await?;Ok::<_,AppError>(())
  }.await;
        source.close().await;
        let _ = std::fs::remove_file(image);
        result
    }
    pub async fn purge(&self, db: Arc<TenantDb>, id: Uuid) -> Result<bool, AppError> {
        let started = std::time::Instant::now();
        let result = self.purge_inner(db, id).await;
        self.metrics.historical_finished(
            "archive_purge",
            result.is_ok(),
            started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        );
        result
    }
    async fn purge_inner(&self, db: Arc<TenantDb>, id: Uuid) -> Result<bool, AppError> {
        let job = get(&self.ledger.pool, id).await?;
        if job.tenant_id != db.tenant_id()
            || job.superseded_at.is_some()
            || !matches!(job.state.as_str(), "VERIFIED" | "PURGING")
        {
            return Err(AppError::Conflict(
                "Purge requires verified archive evidence".into(),
            ));
        }
        let p99 = db.foreground_write_p99_micros();
        if p99 > job.oltp_p99_target_milliseconds as u64 * 1000 {
            sqlx::query("UPDATE archive_manifests SET batch_rows=GREATEST(1,batch_rows/2),retry_after=clock_timestamp()+INTERVAL '30 seconds',last_error='Foreground write p99 exceeds purge budget' WHERE id=$1").bind(id).execute(&self.ledger.pool).await?;
            return Ok(false);
        }
        let objects: Vec<Object> = serde_json::from_value(job.objects.clone()).map_err(fail)?;
        let sum = checksum(&objects)?;
        if job.checksum_sha256.as_deref() != Some(&sum) {
            return Err(fail("Purge manifest checksum differs"));
        }
        let root = directory(&db, id)?;
        let key = self.keys.read(job.tenant_id)?;
        let source = db.background_read_pool()?;
        let refs = policy::references(&source).await?;
        let schema: i64 = sqlx::query_scalar("PRAGMA schema_version")
            .fetch_one(&source)
            .await?;
        let order = policy::deletion_order(&refs)?;
        let checkpoint:Option<(String,String,i64)>=sqlx::query_as("SELECT manifest_checksum,checkpoint,rows_deleted FROM archive_purge_checkpoints WHERE archive_id=?").bind(id.to_string()).fetch_optional(&source).await?;
        if checkpoint.as_ref().is_some_and(|(hash, _, _)| hash != &sum) {
            return Err(fail("Local purge evidence changed"));
        }
        let mut progress: BTreeMap<String, String> = checkpoint
            .as_ref()
            .map(|(_, p, _)| serde_json::from_str(p).map_err(fail))
            .transpose()?
            .unwrap_or_default();
        let mut processed = checkpoint.map(|(_, _, n)| n).unwrap_or(0);
        for spec in order {
            let object = objects
                .iter()
                .find(|o| o.table == spec.name)
                .ok_or_else(|| fail("Manifest omits a fact table"))?;
            if object.columns != raw::columns(&source, spec.name).await? {
                return Err(AppError::Conflict(
                    "Purge schema changed; replan archive".into(),
                ));
            }
            let path = root.join(format!("verified-{}.parquet", spec.name));
            if !path.exists()
                || crate::replication::snapshot::hash_file(&path)? != object.plaintext_checksum
            {
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
            let _permit = match db.background_jobs() {
                Some(j) => Some(j.acquire(crate::background::Priority::ArchivePurge).await?),
                None => None,
            };
            let incoming = refs
                .iter()
                .filter(|r| r.parent == spec.name)
                .cloned()
                .collect::<Vec<_>>();
            let select = raw::row_select(spec.name, &object.columns, "r.id=?")?;
            let delete = format!("DELETE FROM {} WHERE id=?", raw::identifier(spec.name)?);
            let next = rows.last().unwrap().0.clone();
            let count = rows.len() as i64;
            progress.insert(spec.name.into(), next);
            let progress_json = serde_json::to_string(&progress).map_err(fail)?;
            processed += count;
            let ledger = self.ledger.clone();
            let owned = db.clone();
            let evidence = sum.clone();
            let current_progress = progress_json.clone();
            let expected = job.row_count.unwrap();
            let mut tx=db.with_background_immediate_writer(move|c|Box::pin(async move{
    if sqlx::query_scalar::<_,i64>("PRAGMA schema_version").fetch_one(&mut *c).await?!=schema{return Err(AppError::Conflict("Purge schema changed during admission".into()));}
    let mut tx=ledger.pool.begin().await?;ledger.lock_owner(&owned,&mut tx).await?;
    let admitted:bool=sqlx::query_scalar("SELECT state IN ('VERIFIED','PURGING') AND superseded_at IS NULL AND checksum_sha256=$2 FROM archive_manifests WHERE id=$1 FOR UPDATE").bind(id).bind(&evidence).fetch_one(&mut *tx).await?;if !admitted{return Err(AppError::Conflict("Archive evidence no longer authorizes purge".into()));}
    let previous:Option<(String,i64)>=sqlx::query_as("SELECT manifest_checksum,rows_deleted FROM archive_purge_checkpoints WHERE archive_id=?").bind(id.to_string()).fetch_optional(&mut *c).await?;if previous.is_some_and(|(hash,n)|hash!=evidence||n!=processed-count){return Err(AppError::Conflict("Purge checkpoint changed".into()));}
    for (key,payload,hash) in rows{
     if raw::row_hash(&payload)!=hash{return Err(fail("Archived row checksum differs"));}
     let live=sqlx::query(&select).bind(&key).fetch_optional(&mut *c).await?.map(|r|r.get::<String,_>("payload"));
     if live.as_ref().is_some_and(|value|value!=&payload){return Err(AppError::Conflict("Archived row was corrected after verification; replan".into()));}
     if live.is_some(){for reference in &incoming{let exists:bool=sqlx::query_scalar(&format!("SELECT EXISTS(SELECT 1 FROM {} WHERE {}=?)",raw::identifier(&reference.child)?,raw::identifier(&reference.column)?)).bind(&key).fetch_one(&mut *c).await?;if exists{return Err(AppError::Conflict(format!("Archive row remains referenced by {}",reference.child)));}}
      sqlx::query(&delete).bind(&key).execute(&mut *c).await?;
     }
    }
    if processed>expected{return Err(fail("Purge progress exceeds verified rows"));}
    sqlx::query("INSERT INTO archive_purge_checkpoints(archive_id,manifest_checksum,checkpoint,rows_deleted,updated_at) VALUES(?,?,?,?,?) ON CONFLICT(archive_id) DO UPDATE SET checkpoint=excluded.checkpoint,rows_deleted=excluded.rows_deleted,updated_at=excluded.updated_at").bind(id.to_string()).bind(&evidence).bind(&current_progress).bind(processed).bind(crate::time::format_sqlite_timestamp(&Utc::now()).map_err(fail)?).execute(c).await?;
    sqlx::query("UPDATE archive_manifests SET state='PURGING' WHERE id=$1 AND state='VERIFIED'").bind(id).execute(&mut *tx).await?;Ok(tx)
   })).await?;
            sqlx::query("UPDATE archive_manifests SET rows_purged=GREATEST(rows_purged,$2),purge_checkpoint=CASE WHEN rows_purged<=$2 THEN $3 ELSE purge_checkpoint END,last_error=NULL,updated_at=clock_timestamp() WHERE id=$1").bind(id).bind(processed).bind(serde_json::from_str::<serde_json::Value>(&progress_json).map_err(fail)?).execute(&mut *tx).await?;
            tx.commit().await?;
            tokio::time::sleep(Duration::from_millis(10)).await;
            return Ok(false);
        }
        if processed != job.row_count.unwrap() {
            return Err(fail(
                "Archive progress does not cover its complete verified dataset",
            ));
        }
        let ledger = self.ledger.clone();
        let owned = db.clone();
        let evidence = sum.clone();
        let mut tx=db.with_background_immediate_writer(move|c|Box::pin(async move{
   let mut tx=ledger.pool.begin().await?;ledger.lock_owner(&owned,&mut tx).await?;let admitted:bool=sqlx::query_scalar("SELECT state IN ('VERIFIED','PURGING') AND superseded_at IS NULL AND checksum_sha256=$2 FROM archive_manifests WHERE id=$1 FOR UPDATE").bind(id).bind(&evidence).fetch_one(&mut *tx).await?;if !admitted{return Err(AppError::Conflict("Archive changed before completion".into()));}
   sqlx::query("INSERT INTO archive_purge_checkpoints(archive_id,manifest_checksum,rows_deleted,completed,updated_at) VALUES(?,?,?,1,?) ON CONFLICT(archive_id) DO UPDATE SET completed=1,updated_at=excluded.updated_at").bind(id.to_string()).bind(evidence).bind(processed).bind(crate::time::format_sqlite_timestamp(&Utc::now()).map_err(fail)?).execute(c).await?;sqlx::query("UPDATE archive_manifests SET state='PURGING' WHERE id=$1 AND state='VERIFIED'").bind(id).execute(&mut *tx).await?;Ok(tx)
  })).await?;
        sqlx::query("UPDATE archive_manifests SET state='COMPLETE',rows_purged=row_count,purged_at=clock_timestamp(),last_error=NULL,updated_at=clock_timestamp() WHERE id=$1").bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        std::fs::remove_dir_all(root).map_err(fail)?;
        Ok(true)
    }
}
impl Worker {
    pub async fn advance(&self, db: Arc<TenantDb>, id: Uuid) -> Result<bool, AppError> {
        let mut serial = self.ledger.pool.begin().await?;
        let locked: bool =
            sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
                .bind(format!("archive:{id}"))
                .fetch_one(&mut *serial)
                .await?;
        if !locked {
            return Ok(false);
        }
        let job = get(&self.ledger.pool, id).await?;
        let done = match job.state.as_str() {
            "PLANNED" | "EXPORTING" => {
                self.export(db, id).await?;
                false
            }
            "UPLOADED" => {
                self.verify(db, id).await?;
                false
            }
            "VERIFIED" | "PURGING" => {
                self.handoff(db.clone(), id).await?;
                self.purge(db, id).await?
            }
            "COMPLETE" => {
                self.handoff(db, id).await?;
                true
            }
            _ => return Err(fail("Unknown archive phase")),
        };
        serial.rollback().await?;
        Ok(done)
    }
    pub fn spawn(self: Arc<Self>, manager: Arc<crate::tenancy::TenantDbManager>) {
        tokio::spawn(async move {
            let mut ticks = tokio::time::interval(Duration::from_secs(1));
            ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticks.tick().await;
                let jobs=sqlx::query_as::<_,Job>("SELECT a.* FROM archive_manifests a JOIN tenants t ON t.id=a.tenant_id WHERE t.owner_cell=$1 AND t.state='ACTIVE' AND (a.state<>'COMPLETE' OR a.hot_cleaned_at IS NULL) AND a.superseded_at IS NULL AND a.retry_after<=clock_timestamp() ORDER BY a.created_at LIMIT 8").bind(self.ledger.cell_id).fetch_all(&self.ledger.pool).await;
                let jobs = match jobs {
                    Ok(j) => j,
                    Err(error) => {
                        tracing::warn!(%error,"Archive queue unavailable");
                        continue;
                    }
                };
                for job in jobs {
                    let result = async {
                        let db = manager.open(job.tenant_id).await?;
                        self.advance(db, job.id).await
                    }
                    .await;
                    if let Err(error) = result {
                        tracing::warn!(archive=%job.id,%error,"Archive remains retryable");
                        let _=sqlx::query("UPDATE archive_manifests SET last_error=$2,retry_after=clock_timestamp()+INTERVAL '30 seconds' WHERE id=$1 AND state<>'COMPLETE'").bind(job.id).bind(error.to_string()).execute(&self.ledger.pool).await;
                    }
                }
            }
        });
    }
}

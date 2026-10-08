//! Run only in the dedicated export executable, never in the API process.
use super::{
    exports::{self, Filters, Job},
    handoff, hot,
    objects::{self, Object},
    raw,
};
use crate::{
    error::AppError,
    metrics::Metrics,
    replication::{crypto::TenantKeys, snapshot},
    tenancy::analytics_snapshot::{Table, TABLES},
};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use uuid::Uuid;
fn fail(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("Historical export: {e}"))
}
#[derive(Clone, Serialize, Deserialize)]
struct Input {
    object: Object,
    rank: i64,
    tombstones: bool,
    table: String,
}
struct Staging(PathBuf);
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
pub struct Worker {
    pub pool: PgPool,
    pub cell: Uuid,
    pub store: Arc<dyn object_store::ObjectStore>,
    pub keys: TenantKeys,
    pub staging_root: PathBuf,
    pub metrics: Arc<Metrics>,
}
fn spec(name: &str) -> Result<&'static Table, AppError> {
    TABLES
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| fail("Unknown export table"))
}
fn parent(table: &str) -> Option<(&'static str, &'static str)> {
    match table {
        "transaction_lines" => Some(("transactions", "transaction_id")),
        "credit_settlement_items" => Some(("credit_settlements", "settlement_id")),
        "stock_receipt_lines" => Some(("stock_receipts", "receipt_id")),
        "stock_waste_lines" => Some(("stock_waste_events", "waste_event_id")),
        _ => None,
    }
}
fn operational(table: &str) -> &str {
    if table == "transaction_lines" {
        "transaction_products"
    } else {
        table
    }
}
impl Worker {
    async fn owned(&self, id: Uuid, token: Uuid) -> Result<Job, AppError> {
        sqlx::query_as("SELECT e.* FROM historical_exports e JOIN tenants t ON t.id=e.tenant_id WHERE t.state IN ('ACTIVE','COLD') AND e.id=$1 AND e.worker_cell=$2 AND e.worker_token=$3 AND e.worker_expires_at>clock_timestamp() AND e.expires_at>clock_timestamp() AND e.state IN ('PREPARING','SCANNING_ARCHIVE','GENERATING','UPLOADING')")
  .bind(id).bind(self.cell).bind(token).fetch_optional(&self.pool).await?.ok_or_else(||AppError::Forbidden("Export worker was fenced".into()))
    }
    async fn phase(&self, id: Uuid, token: Uuid, next: &str) -> Result<(), AppError> {
        let job = self.owned(id, token).await?;
        let stages = ["PREPARING", "SCANNING_ARCHIVE", "GENERATING", "UPLOADING"];
        let current = stages
            .iter()
            .position(|s| *s == job.state)
            .ok_or_else(|| fail("Unexpected export phase"))?;
        let target = stages
            .iter()
            .position(|s| *s == next)
            .ok_or_else(|| fail("Unexpected target phase"))?;
        if current >= target {
            return Ok(());
        }
        if target != current + 1 {
            return Err(fail("Skipped export phase"));
        }
        let n=sqlx::query("UPDATE historical_exports SET state=$3 WHERE id=$1 AND worker_token=$2 AND worker_expires_at>clock_timestamp() AND state=$4").bind(id).bind(token).bind(next).bind(job.state).execute(&self.pool).await?.rows_affected();
        if n != 1 {
            return Err(AppError::Forbidden("Export ownership changed".into()));
        }
        Ok(())
    }
    pub async fn run(&self, id: Uuid, token: Uuid) -> Result<(), AppError> {
        let started = std::time::Instant::now();
        let mut serial = self.pool.begin().await?;
        let locked: bool =
            sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
                .bind(format!("historical-export-worker:{id}"))
                .fetch_one(&mut *serial)
                .await?;
        if !locked {
            return Err(AppError::Conflict(
                "Another process is executing this export".into(),
            ));
        }
        let result = self.run_inner(id, token).await;
        self.metrics.historical_finished(
            "historical_export",
            result.is_ok(),
            started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        );
        result
    }
    async fn run_inner(&self, id: Uuid, token: Uuid) -> Result<(), AppError> {
        let job = self.owned(id, token).await?;
        let filters: Filters = serde_json::from_value(job.filters.clone()).map_err(fail)?;
        exports::validate(&exports::Request {
            start: job.period_start,
            end: job.period_end,
            format: job.format.clone(),
            filters: filters.clone(),
        })?;
        let timezone: String = sqlx::query_scalar(
            "SELECT timezone FROM tenants WHERE id=$1 AND state IN ('ACTIVE','COLD')",
        )
        .bind(job.tenant_id)
        .fetch_one(&self.pool)
        .await?;
        let zone = timezone.parse::<Tz>().map_err(fail)?;
        let root = self.staging_root.join(format!("{id}-{token}"));
        std::fs::create_dir_all(&root).map_err(fail)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                .map_err(fail)?;
        }
        std::fs::write(
            root.join("export-owner.json"),
            serde_json::to_vec(&serde_json::json!({"id":id,"token":token})).map_err(fail)?,
        )
        .map_err(fail)?;
        let _staging = Staging(root.clone());
        let key = self.keys.read(job.tenant_id)?;
        self.phase(id, token, "SCANNING_ARCHIVE").await?;
        let mut names = vec![filters.table.clone()];
        if let Some((name, _)) = parent(&filters.table) {
            names.push(name.into());
        }
        let mut month = hot::month(job.period_start.date_naive(), -1)?;
        let last = hot::month(
            (job.period_end - chrono::Duration::microseconds(1)).date_naive(),
            1,
        )?;
        let mut inputs = vec![];
        let mut files = vec![];
        let mut coverage: Vec<(DateTime<Utc>, DateTime<Utc>)> = vec![];
        while month <= last {
            self.owned(id, token).await?;
            let mut reader = self.pool.begin().await?;
            handoff::lock_month(&mut reader, job.tenant_id, month, true).await?;
            let archive_rows=sqlx::query("SELECT revision,timezone,period_end,analytics_objects,analytics_checksum_sha256,analytics_projection_version,objects FROM archive_manifests WHERE tenant_id=$1 AND period_start=$2 AND state IN ('VERIFIED','PURGING','COMPLETE') ORDER BY revision")
    .bind(job.tenant_id).bind(month).fetch_all(&mut *reader).await?;
            let mut archives = vec![];
            for archive in archive_rows {
                let calendar = archive
                    .get::<String, _>("timezone")
                    .parse::<Tz>()
                    .map_err(fail)?;
                let from = crate::analytics::calendar::boundary(month, calendar)?;
                let until =
                    crate::analytics::calendar::boundary(archive.get("period_end"), calendar)?;
                if from < job.period_end && until > job.period_start {
                    coverage.push((from, until));
                    archives.push(archive);
                }
            }
            let mut monthly = vec![];
            if !archives.is_empty() {
                for archive in archives {
                    let rank: i64 = archive.get("revision");
                    let projection: Vec<Object> =
                        serde_json::from_value(archive.get("analytics_objects")).map_err(fail)?;
                    if projection.is_empty() {
                        return Err(AppError::Conflict("Legacy archive requires backfill and a new canonical projection before export".into()));
                    }
                    let hash = raw::row_hash(&serde_json::to_string(&projection).map_err(fail)?);
                    if archive
                        .get::<Option<String>, _>("analytics_checksum_sha256")
                        .as_deref()
                        != Some(&hash)
                        || archive.get::<Option<i64>, _>("analytics_projection_version")
                            != Some(crate::tenancy::analytics_snapshot::SNAPSHOT_VERSION as i64)
                    {
                        return Err(fail("Archive projection evidence differs"));
                    }
                    let raw_objects: Vec<Object> =
                        serde_json::from_value(archive.get("objects")).map_err(fail)?;
                    for name in &names {
                        let object = projection
                            .iter()
                            .find(|o| &o.table == name)
                            .ok_or_else(|| fail("Archive omits requested table"))?
                            .clone();
                        monthly.push(Input {
                            object,
                            rank,
                            tombstones: false,
                            table: name.clone(),
                        });
                        if let Some(object) = raw_objects.iter().find(|o| {
                            o.table == operational(name)
                                && o.columns.iter().any(|c| c.name == "deleted_at")
                        }) {
                            monthly.push(Input {
                                object: object.clone(),
                                rank,
                                tombstones: true,
                                table: name.clone(),
                            });
                        }
                    }
                }
            } else {
                let from = crate::analytics::calendar::boundary(month, zone)?;
                let until = crate::analytics::calendar::boundary(hot::month(month, 1)?, zone)?;
                if from >= job.period_end || until <= job.period_start {
                    reader.rollback().await?;
                    month = hot::month(month, 1)?;
                    continue;
                }
                let hot:Option<(serde_json::Value,i64,String)>=sqlx::query_as("SELECT objects,projection_version,timezone FROM hot_month_manifests WHERE tenant_id=$1 AND period_start=$2 AND state='READY'").bind(job.tenant_id).bind(month).fetch_optional(&mut *reader).await?;
                let Some((value, version, calendar)) = hot else {
                    reader.rollback().await?;
                    month = hot::month(month, 1)?;
                    continue;
                };
                if version != crate::tenancy::analytics_snapshot::SNAPSHOT_VERSION as i64
                    || calendar != timezone
                {
                    return Err(AppError::Conflict(
                        "Hot calendar or projection changed; rebuild it".into(),
                    ));
                }
                coverage.push((from, until));
                let objects: Vec<Object> = serde_json::from_value(value).map_err(fail)?;
                for name in &names {
                    let object = objects
                        .iter()
                        .find(|o| &o.table == name)
                        .ok_or_else(|| fail("Hot manifest omits requested table"))?
                        .clone();
                    monthly.push(Input {
                        object,
                        rank: i64::MAX,
                        tombstones: false,
                        table: name.clone(),
                    });
                }
            }
            // Hold the shared lock until inputs are local; retirement can then delete their remote hot copies.
            for input in monthly {
                let path = root.join(format!("input-{}.parquet", inputs.len()));
                objects::download(self.store.as_ref(), &key, &input.object, &path).await?;
                files.push(path);
                inputs.push(input);
            }
            reader.commit().await?;
            month = hot::month(month, 1)?;
        }
        coverage.sort_unstable();
        let mut covered = job.period_start;
        for (from, until) in coverage {
            if from > covered {
                break;
            }
            if until > covered {
                covered = until;
            }
        }
        if covered < job.period_end {
            return Err(AppError::Conflict("Requested range has no complete verified object coverage; publish hot data or archive the missing months".into()));
        }
        let n=sqlx::query("UPDATE historical_exports SET source_objects=$3 WHERE id=$1 AND worker_token=$2 AND worker_expires_at>clock_timestamp() AND state IN ('SCANNING_ARCHIVE','GENERATING','UPLOADING')")
   .bind(id).bind(token).bind(serde_json::to_value(&inputs).map_err(fail)?).execute(&self.pool).await?.rows_affected();
        if n != 1 {
            return Err(AppError::Forbidden(
                "Export ownership changed during download".into(),
            ));
        }
        self.phase(id, token, "GENERATING").await?;
        let output = root.join(match job.format.as_str() {
            "CSV" => "result.csv",
            "CSV_GZ" => "result.csv.gz",
            _ => "result.parquet",
        });
        let request = exports::Request {
            start: job.period_start,
            end: job.period_end,
            format: job.format.clone(),
            filters,
        };
        let query_root = root.clone();
        let query_output = output.clone();
        let rows = tokio::task::spawn_blocking(move || {
            generate(&query_root, &query_output, &request, &inputs, &files)
        })
        .await
        .map_err(fail)??;
        self.phase(id, token, "UPLOADING").await?;
        self.owned(id, token).await?;
        let object_key = format!(
            "tenants/{}/exports/{}/{}-{}",
            job.tenant_id,
            id,
            token,
            output.file_name().unwrap().to_string_lossy()
        );
        let object = objects::upload(
            self.store.as_ref(),
            &key,
            object_key,
            "export".into(),
            rows,
            snapshot::hash_file(&output)?,
            &output,
        )
        .await?;
        let n=sqlx::query("UPDATE historical_exports SET state='READY',result_object_key=$3,checksum_sha256=$4,result_size_bytes=$5,row_count=$6,result_object=$7,ready_at=clock_timestamp(),last_error=NULL WHERE id=$1 AND worker_token=$2 AND worker_expires_at>clock_timestamp() AND expires_at>clock_timestamp() AND state='UPLOADING' AND EXISTS(SELECT 1 FROM tenants WHERE id=tenant_id AND state IN ('ACTIVE','COLD'))")
   .bind(id).bind(token).bind(&object.key).bind(&object.plaintext_checksum).bind(object.plaintext_bytes as i64).bind(rows).bind(serde_json::to_value(&object).map_err(fail)?).execute(&self.pool).await?.rows_affected();
        if n != 1 {
            return Err(AppError::Forbidden("Export publication was fenced".into()));
        }
        Ok(())
    }
}
fn generate(
    root: &Path,
    output: &Path,
    request: &exports::Request,
    inputs: &[Input],
    files: &[PathBuf],
) -> Result<i64, AppError> {
    let db = duckdb::Connection::open_in_memory().map_err(fail)?;
    db.execute_batch(&format!(
        "SET threads=1;SET memory_limit='128MB';SET TimeZone='UTC';SET temp_directory={}",
        raw::literal(root.join("duck-temp").to_str().unwrap())
    ))
    .map_err(fail)?;
    let mut names = vec![request.filters.table.as_str()];
    if let Some((name, _)) = parent(&request.filters.table) {
        names.push(name);
    }
    for name in names {
        let table = spec(name)?;
        let union = inputs
            .iter()
            .zip(files)
            .filter(|(i, _)| i.table == name && !i.tombstones)
            .map(|(i, path)| {
                format!(
                    "SELECT *,{}::BIGINT AS __rank FROM read_parquet({})",
                    i.rank,
                    raw::literal(path.to_str().unwrap())
                )
            })
            .collect::<Vec<_>>()
            .join(" UNION ALL ");
        if union.is_empty() {
            return Err(fail("Export has no verified source objects"));
        }
        let raw_union = inputs
            .iter()
            .zip(files)
            .filter(|(i, _)| i.table == name && i.tombstones)
            .map(|(i, path)| {
                format!(
                    "SELECT id,payload,{}::BIGINT AS __rank FROM read_parquet({})",
                    i.rank,
                    raw::literal(path.to_str().unwrap())
                )
            })
            .collect::<Vec<_>>()
            .join(" UNION ALL ");
        let tombstones = if raw_union.is_empty() {
            String::new()
        } else {
            db.execute_batch(&format!("CREATE VIEW tombstones_{name} AS SELECT * FROM ({raw_union}) QUALIFY row_number() OVER(PARTITION BY id ORDER BY __rank DESC)=1")).map_err(fail)?;
            format!(" AND NOT EXISTS(SELECT 1 FROM tombstones_{name} t WHERE t.id=CAST(f.id AS VARCHAR) AND t.__rank>=f.__rank AND json_extract_string(t.payload,'$.deleted_at') IS NOT NULL)")
        };
        let columns = table
            .columns
            .iter()
            .map(|c| format!("f.{}", c.name))
            .collect::<Vec<_>>()
            .join(",");
        db.execute_batch(&format!("CREATE VIEW {name} AS SELECT {columns} FROM (SELECT * FROM ({union}) QUALIFY row_number() OVER(PARTITION BY id ORDER BY __rank DESC)=1) f WHERE true{tombstones}")).map_err(fail)?;
    }
    let selected = spec(&request.filters.table)?;
    let (time, from) = if let Some(time) = selected.time {
        (format!("r.{time}"), format!("{} r", selected.name))
    } else {
        let (parent_name, key) =
            parent(selected.name).ok_or_else(|| fail("Missing export parent"))?;
        (
            format!(
                "p.{}",
                spec(parent_name)?
                    .time
                    .ok_or_else(|| fail("Missing parent timestamp"))?
            ),
            format!("{} r JOIN {parent_name} p ON p.id=r.{key}", selected.name),
        )
    };
    let mut filter = format!(
        "{time}>=TIMESTAMPTZ {} AND {time}<TIMESTAMPTZ {}",
        raw::literal(&crate::time::format_sqlite_timestamp(&request.start).map_err(fail)?),
        raw::literal(&crate::time::format_sqlite_timestamp(&request.end).map_err(fail)?)
    );
    if let Some(location) = request.filters.location_id {
        filter.push_str(&format!(
            " AND r.location_id=UUID {}",
            raw::literal(&location.to_string())
        ));
    }
    let query = if request.filters.daily_revenue {
        format!("SELECT CAST(r.occurred_at AT TIME ZONE 'UTC' AS DATE) AS utc_date,COUNT(*) AS transaction_count,SUM(r.amount) AS amount,SUM(r.paid_amount) AS paid_amount FROM {from} WHERE {filter} AND r.payment_status='completed' GROUP BY 1 ORDER BY 1")
    } else {
        format!("SELECT r.* FROM {from} WHERE {filter} ORDER BY r.id")
    };
    let rows: i64 = db
        .query_row(&format!("SELECT COUNT(*) FROM ({query})"), [], |r| r.get(0))
        .map_err(fail)?;
    let options = match request.format.as_str() {
        "CSV" => "FORMAT CSV,HEADER true",
        "CSV_GZ" => "FORMAT CSV,HEADER true,COMPRESSION GZIP",
        _ => "FORMAT PARQUET,COMPRESSION ZSTD",
    };
    db.execute_batch(&format!(
        "COPY ({query}) TO {} ({options})",
        raw::literal(output.to_str().unwrap())
    ))
    .map_err(fail)?;
    Ok(rows)
}

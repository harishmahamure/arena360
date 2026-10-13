//! Consistent SQLite snapshot -> shadow DuckDB -> local outbox replay -> switch.
use super::{
    consumer::{self, BatchOutcome},
    tenant_db::{error, TenantAnalytics},
};
use crate::{
    error::AppError,
    tenancy::{
        analytics_snapshot::{normalize, TABLES},
        TenantDb,
    },
};
use chrono::NaiveDate;
use chrono_tz::Tz;
use duckdb::params;
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

pub(crate) struct RebuildFiles {
    pub root: PathBuf,
}
impl Drop for RebuildFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn failure(message: &str) -> AppError {
    AppError::Internal(format!("Analytics rebuild: {message}"))
}
fn literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
pub(crate) use super::calendar::boundary;
/// Durable AUTOINCREMENT watermark survives local retention of every event row.
pub async fn source_watermark(db: &TenantDb) -> Result<i64, AppError> {
    db.ensure_current_owner()?;
    let sequence = sqlx::query_scalar(
        "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='outbox_events'),0)",
    )
    .fetch_one(&db.background_read_pool()?)
    .await?;
    db.ensure_current_owner()?;
    Ok(sequence)
}
/// A read-only VACUUM snapshot does not take the foreground application writer.
async fn snapshot(db: &TenantDb, files: &RebuildFiles) -> Result<i64, AppError> {
    db.ensure_current_owner()?;
    let path = files.root.join("source.sqlite");
    sqlx::query("VACUUM INTO ?")
        .bind(
            path.to_str()
                .ok_or_else(|| failure("invalid snapshot path"))?,
        )
        .execute(&db.background_read_pool()?)
        .await?;
    db.ensure_current_owner()?;
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(path)
                .read_only(true),
        )
        .await?;
    let result = sqlx::query_scalar(
        "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='outbox_events'),0)",
    )
    .fetch_one(&pool)
    .await;
    pool.close().await;
    db.ensure_current_owner()?;
    Ok(result?)
}
/// Recalculate bounded hot-month aggregates. All-locations visitors are computed
/// distinctly, never summed from venue rows. Old summaries are preserved separately.
pub fn refresh_monthly(tx: &duckdb::Transaction<'_>) -> Result<(), AppError> {
    let start: String = tx
        .query_row(
            "SELECT CAST(hot_window_start AS VARCHAR) FROM _ingest_state",
            [],
            |r| r.get(0),
        )
        .map_err(error)?;
    refresh_monthly_from(
        tx,
        start
            .parse()
            .map_err(|_| failure("invalid summary month"))?,
    )
}
pub(crate) fn refresh_monthly_from(
    tx: &duckdb::Transaction<'_>,
    start: NaiveDate,
) -> Result<(), AppError> {
    // The literal is generated from a typed date, never from SQL supplied by a caller.
    let sql = MONTHLY_SQL.replace("__START__", &format!("DATE '{start}'"));
    tx.execute_batch(&sql).map_err(error)
}
const MONTHLY_SQL: &str = r#"
DELETE FROM monthly_summary WHERE month >= __START__;
INSERT INTO monthly_summary
WITH sales AS (
 SELECT CAST(date_trunc('month',local_date) AS DATE) AS month,location_id,
        sum(amount) revenue,
        sum(CASE WHEN transaction_type='plan_purchase' THEN amount ELSE 0 END) plan_revenue,
        sum(CASE WHEN transaction_type='product_purchase' THEN amount ELSE 0 END) pos_revenue,
        count(*) transactions
 FROM transactions WHERE is_booked AND local_date>=__START__
 GROUP BY GROUPING SETS ((month,location_id),(month))
 HAVING GROUPING(location_id)=1 OR location_id IS NOT NULL
), starts AS (
 SELECT CAST(date_trunc('month',start_local_date) AS DATE) AS month,location_id,
        count(*) session_starts,count(DISTINCT player_id) visitors
 FROM sessions WHERE NOT is_staff_allowance AND start_local_date>=__START__ GROUP BY GROUPING SETS ((month,location_id),(month))
 HAVING GROUPING(location_id)=1 OR location_id IS NOT NULL
), hours AS (
 SELECT CAST(date_trunc('month',local_date) AS DATE) AS month,location_id,
        sum(occupied_seconds)/3600.0 occupied_hours
 FROM session_hours WHERE local_date>=__START__ AND session_id IN (SELECT id FROM sessions WHERE NOT is_staff_allowance) GROUP BY GROUPING SETS ((month,location_id),(month))
 HAVING GROUPING(location_id)=1 OR location_id IS NOT NULL
), keys AS (
 SELECT month,coalesce(location_id,'00000000-0000-0000-0000-000000000000'::UUID) location_id FROM sales
 UNION SELECT month,coalesce(location_id,'00000000-0000-0000-0000-000000000000'::UUID) FROM starts
 UNION SELECT month,coalesce(location_id,'00000000-0000-0000-0000-000000000000'::UUID) FROM hours
)
SELECT k.month,k.location_id,coalesce(s.revenue,0),coalesce(s.plan_revenue,0),coalesce(s.pos_revenue,0),
       coalesce(s.transactions,0),coalesce(t.session_starts,0),coalesce(h.occupied_hours,0),coalesce(t.visitors,0)
FROM keys k
LEFT JOIN sales s ON s.month=k.month AND coalesce(s.location_id,'00000000-0000-0000-0000-000000000000'::UUID)=k.location_id
LEFT JOIN starts t ON t.month=k.month AND coalesce(t.location_id,'00000000-0000-0000-0000-000000000000'::UUID)=k.location_id
LEFT JOIN hours h ON h.month=k.month AND coalesce(h.location_id,'00000000-0000-0000-0000-000000000000'::UUID)=k.location_id
WHERE k.month >= __START__;
"#;
#[derive(Debug)]
struct MonthlyRow {
    month: String,
    location: String,
    revenue: String,
    plan: String,
    pos: String,
    transactions: i64,
    starts: i64,
    hours: f64,
    visitors: i64,
}
async fn preserve_summaries(
    live: &Arc<TenantAnalytics>,
    shadow: &Arc<TenantAnalytics>,
    timezone: &str,
) -> Result<(), AppError> {
    let hot = shadow
        .read(|tx| {
            tx.query_row(
                "SELECT CAST(hot_window_start AS VARCHAR) FROM _ingest_state",
                [],
                |r| r.get::<_, String>(0),
            )
            .map_err(error)
        })
        .await?;
    let mut offset = 0;
    loop {
        let hot = hot.clone();
        let timezone = timezone.to_owned();
        let rows=live.read(move|tx| {
            let stored:String=tx.query_row("SELECT timezone FROM _ingest_state",[],|r|r.get(0)).map_err(error)?;
            if stored!=timezone {return Ok(Vec::new());}
            let mut q=tx.prepare("SELECT CAST(month AS VARCHAR),CAST(location_id AS VARCHAR),CAST(revenue AS VARCHAR),CAST(plan_revenue AS VARCHAR),CAST(pos_revenue AS VARCHAR),transactions,session_starts,occupied_hours,visitors FROM monthly_summary WHERE month<CAST(? AS DATE) ORDER BY month,location_id LIMIT 500 OFFSET ?").map_err(error)?;
            let rows=q.query_map(params![hot,offset],|r|Ok(MonthlyRow{month:r.get(0)?,location:r.get(1)?,revenue:r.get(2)?,plan:r.get(3)?,pos:r.get(4)?,transactions:r.get(5)?,starts:r.get(6)?,hours:r.get(7)?,visitors:r.get(8)?})).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
            Ok(rows)
        }).await?;
        if rows.is_empty() {
            break;
        }
        offset += rows.len() as i64;
        shadow.write(move|tx|{for r in rows {tx.execute("INSERT INTO monthly_summary VALUES(CAST(? AS DATE),CAST(? AS UUID),CAST(? AS DECIMAL(19,4)),CAST(? AS DECIMAL(19,4)),CAST(? AS DECIMAL(19,4)),?,?,?,?)",params![r.month,r.location,r.revenue,r.plan,r.pos,r.transactions,r.starts,r.hours,r.visitors]).map_err(error)?;}Ok(())}).await?;
    }
    Ok(())
}
async fn backfill(
    shadow: Arc<TenantAnalytics>,
    files: Arc<RebuildFiles>,
    timezone: String,
    t0: i64,
) -> Result<(), AppError> {
    let zone = timezone
        .parse::<Tz>()
        .map_err(|_| failure("invalid timezone"))?;
    let extensions = std::env::var("DUCKDB_EXTENSION_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| files.root.parent().unwrap().join(".duckdb_extensions"));
    std::fs::create_dir_all(&extensions).map_err(|e| failure(&e.to_string()))?;
    shadow.write(move|tx| {
        let hot:String=tx.query_row("SELECT CAST(hot_window_start AS VARCHAR) FROM _ingest_state",[],|r|r.get(0)).map_err(error)?;
        let cutoff=crate::time::format_sqlite_timestamp(&boundary(hot.parse().map_err(|_|failure("invalid hot month"))?,zone)?).map_err(|e|failure(&e.to_string()))?;
        tx.execute_batch(&format!("SET extension_directory={}; INSTALL sqlite FROM 'https://extensions.duckdb.org'; LOAD sqlite; ATTACH {} AS source (TYPE SQLITE,READ_ONLY)",literal(extensions.to_str().ok_or_else(||failure("invalid extension path"))?),literal(files.root.join("source.sqlite").to_str().ok_or_else(||failure("invalid snapshot path"))?))).map_err(error)?;
        // Each source query is explicitly allowlisted and executes against the one
        // immutable snapshot. JSON integers remain decimal text through conversion.
        for spec in TABLES {
            let mut select=spec.select();
            if let Some(time)=spec.time {
                let column=spec.columns.iter().find(|c|c.name==time).ok_or_else(||failure("missing retention source"))?;
                let condition=match spec.name {
                    "sessions"=>format!("({} >= {} OR r.end_time IS NULL OR r.end_time > {})",column.source,literal(&cutoff),literal(&cutoff)),
                    "shifts"=>format!("({} >= {} OR r.clock_out IS NULL OR r.clock_out > {})",column.source,literal(&cutoff),literal(&cutoff)),
                    _=>format!("({} IS NULL OR {} >= {})",column.source,column.source,literal(&cutoff)),
                };
                select.push_str(&format!(" AND {condition}"));
            }
            if let Some(parent_key)=spec.parent {
                let parent_name=match spec.name {"transaction_lines"=>"transactions","credit_settlement_items"=>"credit_settlements","stock_receipt_lines"=>"stock_receipts","stock_waste_lines"=>"stock_waste_events",_=>return Err(failure("unknown source parent"))};
                let parent=TABLES.iter().find(|p|p.name==parent_name).ok_or_else(||failure("missing source parent"))?;
                let mut predicate=parent.predicate.to_owned();
                if let Some(time)=parent.time {
                    let c=parent.columns.iter().find(|c|c.name==time).ok_or_else(||failure("missing parent retention source"))?;
                    predicate.push_str(&format!(" AND ({} IS NULL OR {} >= {})",c.source,c.source,literal(&cutoff)));
                }
                select.push_str(&format!(" AND r.{parent_key} IN (SELECT r.id FROM {} WHERE {predicate})",parent.source));
            }
            tx.execute_batch(&format!("CREATE TEMP TABLE _backfill_raw AS SELECT row_number() OVER () AS row_no,payload FROM sqlite_query('source',{})",literal(&select))).map_err(error)?;
            let count:i64=tx.query_row("SELECT count(*) FROM _backfill_raw",[],|r|r.get(0)).map_err(error)?;
            for offset in (0..count).step_by(500) {
                let rows={
                    let mut q=tx.prepare("SELECT row_no,payload FROM _backfill_raw WHERE row_no>? AND row_no<=? ORDER BY row_no").map_err(error)?;
                    let rows=q.query_map(params![offset,offset+500],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
                    rows
                };
                for (number,raw) in rows {
                    let mut row=normalize(spec,serde_json::from_str(&raw).map_err(|e|failure(&e.to_string()))?)?;
                    consumer::derive_labels(spec,&mut row,zone)?;
                    tx.execute("UPDATE _backfill_raw SET payload=? WHERE row_no=?",params![serde_json::to_string(&row).map_err(|e|failure(&e.to_string()))?,number]).map_err(error)?;
                }
            }
            let columns=spec.columns.iter().map(|c|c.name).collect::<Vec<_>>().join(",");
            let casts=spec.columns.iter().map(|c|format!("CAST(json_extract_string(payload,'$.{}') AS {})",c.name,consumer::sql_type(c.kind))).collect::<Vec<_>>().join(",");
            let parent=match spec.name {"transaction_lines"=>Some(("transactions","transaction_id")),"credit_settlement_items"=>Some(("credit_settlements","settlement_id")),"stock_receipt_lines"=>Some(("stock_receipts","receipt_id")),"stock_waste_lines"=>Some(("stock_waste_events","waste_event_id")),_=>None};
            let filter=parent.map(|(table,key)|format!(" WHERE CAST(json_extract_string(payload,'$.{key}') AS UUID) IN (SELECT id FROM {table})")).unwrap_or_default();
            tx.execute_batch(&format!("INSERT INTO {}({columns}) SELECT {casts} FROM _backfill_raw{filter}; DROP TABLE _backfill_raw",spec.name)).map_err(error)?;
        }
        let mut last=String::new();
        loop {
            let ids={let mut q=tx.prepare("SELECT CAST(id AS VARCHAR) FROM sessions WHERE end_time IS NOT NULL AND CAST(id AS VARCHAR)>? ORDER BY id LIMIT 500").map_err(error)?;
                let rows=q.query_map(params![last],|r|r.get::<_,String>(0)).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;rows};
            if ids.is_empty(){break;}
            for id in ids {consumer::replace_hours(tx,&id,zone)?;last=id;}
        }
        refresh_monthly(tx)?;
        tx.execute("UPDATE _ingest_state SET last_sequence=?,rebuild_boundary_seq=?,replay_start_sequence=?,status='READY' WHERE id=1",params![t0,t0,Option::<u64>::None]).map_err(error)?;
        Ok(())
    }).await?;
    shadow
        .write(|tx| {
            tx.execute_batch("DETACH source").map_err(error)?;
            Ok(())
        })
        .await
}
/// Catch up through a finite post-snapshot watermark using retained local events.
/// A retention gap fails closed; the next rebuild starts from a newer snapshot.
async fn replay(
    db: Arc<TenantDb>,
    shadow: Arc<TenantAnalytics>,
    barrier: i64,
) -> Result<(), AppError> {
    loop {
        db.ensure_current_owner()?;
        let last = shadow
            .read(|tx| {
                tx.query_row(
                    "SELECT CAST(last_sequence AS BIGINT) FROM _ingest_state",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .map_err(error)
            })
            .await?;
        if last >= barrier {
            return Ok(());
        }
        let events =
            super::events::read_batch(&db, last, barrier, consumer::BATCH_SIZE as i64).await?;
        if events.is_empty() {
            return Err(failure("local outbox replay has a retention gap"));
        }
        match consumer::apply_batch(shadow.clone(), events).await? {
            BatchOutcome::Applied { last_sequence, .. } if last_sequence > last => {}
            _ => {
                return Err(failure(
                    "local outbox replay has a sequence gap or incomplete snapshot",
                ))
            }
        }
    }
}

/// Production rebuild. The canonical file remains REBUILDING until the new file
/// has caught up through a finite post-backfill source watermark.
pub async fn rebuild(
    db: Arc<TenantDb>,
    live: Arc<TenantAnalytics>,
) -> Result<(), AppError> {
    rebuild_with_hook(db, live, || async { Ok(()) }).await
}
/// Hook supports deterministic concurrent-write/failure tests at the snapshot boundary.
pub async fn rebuild_with_hook<F, Fut>(
    db: Arc<TenantDb>,
    live: Arc<TenantAnalytics>,
    after_snapshot: F,
) -> Result<(), AppError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<(), AppError>>,
{
    let _job=match db.background_jobs(){Some(jobs)=>Some(jobs.acquire(crate::background::Priority::HotBackfill).await?),None=>None};
    static REBUILDS: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    let _cell_slot = REBUILDS
        .get_or_init(|| tokio::sync::Semaphore::new(1))
        .acquire()
        .await
        .map_err(|_| failure("rebuild scheduler closed"))?;
    let _serial = live.rebuild_lock.lock().await;
    db.ensure_current_owner()?;
    live.write(|tx| {
        tx.execute_batch("UPDATE _ingest_state SET status='REBUILDING'")
            .map_err(error)?;
        Ok(())
    })
    .await?;
    let files = Arc::new(RebuildFiles {
        root: db
            .path()
            .with_file_name(format!("rebuild-{}", Uuid::new_v4())),
    });
    std::fs::create_dir(&files.root).map_err(|e| failure(&e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&files.root, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| failure(&e.to_string()))?;
    }
    let t0 = snapshot(&db, &files).await?;
    let snapshot_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(files.root.join("source.sqlite"))
                .read_only(true),
        )
        .await?;
    let timezone_result =
        sqlx::query_scalar::<_, String>("SELECT timezone FROM tenant_runtime WHERE singleton=1")
            .fetch_one(&snapshot_pool)
            .await;
    snapshot_pool.close().await;
    let timezone = timezone_result?;
    after_snapshot().await?;
    let shadow = TenantAnalytics::shadow(db.clone(), files.clone(), timezone.clone()).await?;
    preserve_summaries(&live, &shadow, &timezone).await?;
    backfill(shadow.clone(), files, timezone.clone(), t0).await?;
    let barrier = source_watermark(&db).await?;
    replay(db.clone(), shadow.clone(), barrier).await?;
    let current: String =
        sqlx::query_scalar("SELECT timezone FROM tenant_runtime WHERE singleton=1")
            .fetch_one(&db.background_read_pool()?)
            .await?;
    if current != timezone {
        return Err(failure("timezone changed during rebuild"));
    }
    shadow
        .write(|tx| {
            refresh_monthly(tx)?;
            tx.execute_batch(
                "UPDATE _ingest_state SET status='READY',updated_at=current_timestamp",
            )
            .map_err(error)?;
            Ok(())
        })
        .await?;
    live.install(shadow).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime,Utc};
    #[test]
    fn month_boundary_handles_second_offsets_and_skipped_local_dates() {
        assert_eq!(
            boundary(
                NaiveDate::from_ymd_opt(1972, 1, 7).unwrap(),
                chrono_tz::Africa::Monrovia
            )
            .unwrap(),
            "1972-01-07T00:44:30Z".parse::<DateTime<Utc>>().unwrap()
        );
        assert_eq!(
            boundary(
                NaiveDate::from_ymd_opt(2011, 12, 30).unwrap(),
                chrono_tz::Pacific::Apia
            )
            .unwrap(),
            "2011-12-30T10:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
    }
}

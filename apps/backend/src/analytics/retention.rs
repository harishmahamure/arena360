//! Daily tenant-calendar maintenance. Summaries are sealed before expired facts
//! are removed in bounded transactions; the ingest checkpoint never advances.
use super::{
    rebuild::{boundary, refresh_monthly_from},
    tenant_db::{error, TenantAnalytics},
};
use crate::{error::AppError, tenancy::analytics_snapshot::TABLES};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use chrono_tz::Tz;
use duckdb::params;
use std::sync::Arc;
pub const DELETE_BATCH: usize = 1000;
#[derive(Debug, PartialEq)]
pub enum RetentionOutcome {
    Skipped,
    Applied {
        deleted_rows: usize,
        hot_window_start: NaiveDate,
        next_due: DateTime<Utc>,
    },
}
fn invalid() -> AppError {
    AppError::Internal("Invalid analytics retention calendar".into())
}
fn month(date: NaiveDate, offset: i32) -> Result<NaiveDate, AppError> {
    let index = date.year() * 12 + date.month0() as i32 + offset;
    NaiveDate::from_ymd_opt(index.div_euclid(12), index.rem_euclid(12) as u32 + 1, 1)
        .ok_or_else(invalid)
}
// Session/shift intervals overlapping the hot window and unfinished work survive.
fn predicate(table: &str, column: &str, alias: &str) -> String {
    let t = format!("{alias}.{column}");
    match table {
        "sessions"=>format!("{t}<CAST($1 AS TIMESTAMP) AND {alias}.end_time IS NOT NULL AND {alias}.end_time<=CAST($1 AS TIMESTAMP)"),
        "shifts"=>format!("{t}<CAST($1 AS TIMESTAMP) AND {alias}.clock_out IS NOT NULL AND {alias}.clock_out<=CAST($1 AS TIMESTAMP)"),
        _=>format!("{t} IS NOT NULL AND {t}<CAST($1 AS TIMESTAMP)"),
    }
}
async fn purge(
    analytics: &Arc<TenantAnalytics>,
    sql: String,
    cutoff: &str,
) -> Result<usize, AppError> {
    let mut count = 0;
    loop {
        let sql = sql.clone();
        let cutoff = cutoff.to_owned();
        let deleted = analytics
            .write(move |tx| {
                let status: String = tx
                    .query_row("SELECT status FROM _ingest_state", [], |r| r.get(0))
                    .map_err(error)?;
                if status != "READY" {
                    return Err(AppError::Conflict(
                        "Analytics maintenance interrupted by rebuild or lag".into(),
                    ));
                }
                tx.execute(&sql, params![cutoff]).map_err(error)
            })
            .await?;
        count += deleted;
        if deleted == 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    Ok(count)
}
pub async fn run(
    analytics: Arc<TenantAnalytics>,
    now: DateTime<Utc>,
) -> Result<RetentionOutcome, AppError> {
    let _maintenance = analytics.rebuild_lock.lock().await;
    let source = super::rebuild::source_watermark(analytics.owner()).await?;
    let prepared = analytics
        .write(move |tx| {
            let (status, timezone, old): (String, String, String) = tx
                .query_row(
                    "SELECT status,timezone,CAST(hot_window_start AS VARCHAR) FROM _ingest_state",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .map_err(error)?;
            let last: i64 = tx
                .query_row(
                    "SELECT CAST(last_sequence AS BIGINT) FROM _ingest_state",
                    [],
                    |r| r.get(0),
                )
                .map_err(error)?;
            if status != "READY" || last < source {
                return Ok(None);
            }
            let zone = timezone.parse::<Tz>().map_err(|_| invalid())?;
            let local = now.with_timezone(&zone).date_naive();
            let old = old.parse::<NaiveDate>().map_err(|_| invalid())?;
            let hot = month(local, -18)?.max(old);
            // Refresh the expiring month while its complete facts still exist.
            let refresh = if hot > old {
                old.min(month(local, -1)?)
            } else {
                month(local, -1)?
            };
            refresh_monthly_from(tx, refresh)?;
            tx.execute(
                "UPDATE _ingest_state SET hot_window_start=CAST(? AS DATE) WHERE id=1",
                params![hot.to_string()],
            )
            .map_err(error)?;
            let cutoff = crate::time::format_sqlite_timestamp(&boundary(hot, zone)?)
                .map_err(|_| invalid())?;
            let next = boundary(local.succ_opt().ok_or_else(invalid)?, zone)?;
            Ok(Some((hot, cutoff, next)))
        })
        .await?;
    let Some((hot, cutoff, next_due)) = prepared else {
        return Ok(RetentionOutcome::Skipped);
    };
    // Advancing the cutoff first prevents concurrent ingestion from reintroducing
    // expired facts. Readers use this same cutoff; old summaries are already sealed.
    let mut deleted_rows=purge(&analytics,format!("DELETE FROM session_hours WHERE (session_id,hour_start) IN (SELECT session_id,hour_start FROM session_hours WHERE local_date<CAST(? AS DATE) LIMIT {DELETE_BATCH})"),&hot.to_string()).await?;
    for spec in TABLES.iter().filter(|s| s.time.is_some()) {
        let parent_predicate = predicate(spec.name, spec.time.unwrap(), "p");
        let children = match spec.name {
            "transactions" => Some(("transaction_lines", "transaction_id")),
            "credit_settlements" => Some(("credit_settlement_items", "settlement_id")),
            "stock_receipts" => Some(("stock_receipt_lines", "receipt_id")),
            "stock_waste_events" => Some(("stock_waste_lines", "waste_event_id")),
            _ => None,
        };
        if let Some((child, key)) = children {
            deleted_rows+=purge(&analytics,format!("DELETE FROM {child} WHERE id IN (SELECT c.id FROM {child} c JOIN {} p ON p.id=c.{key} WHERE {parent_predicate} LIMIT {DELETE_BATCH})",spec.name),&cutoff).await?;
        }
        deleted_rows+=purge(&analytics,format!("DELETE FROM {} WHERE id IN (SELECT p.id FROM {} p WHERE {parent_predicate} LIMIT {DELETE_BATCH})",spec.name,spec.name),&cutoff).await?;
    }
    Ok(RetentionOutcome::Applied {
        deleted_rows,
        hot_window_start: hot,
        next_due,
    })
}

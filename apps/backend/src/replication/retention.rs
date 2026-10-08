//! Dependency-aware retention. Control-plane tombstones precede remote deletes.
use super::ledger::PostgresLedger;
use crate::{error::AppError, tenancy::TenantDb};
use chrono::{DateTime, Duration, Utc};
use object_store::{path::Path, ObjectStore, ObjectStoreExt};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub id: Uuid,
    pub at: DateTime<Utc>,
    pub capture: u64,
}
#[derive(Debug, Clone)]
pub struct Wal {
    pub number: i64,
    pub last_at: DateTime<Utc>,
    pub last_capture: u64,
}
#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    pub snapshots: Vec<Uuid>,
    pub segments: Vec<i64>,
}
/// Inputs contain only verified, live rows. Sealed generations cannot receive
/// new captures; expiry waits for both their seal and last capture to age out.
pub fn plan(
    snapshots: &[Snapshot],
    wal: &[Wal],
    cutoff: DateTime<Utc>,
    expired_sealed: bool,
) -> Plan {
    if expired_sealed
        && snapshots.iter().all(|s| s.at < cutoff)
        && wal.iter().all(|w| w.last_at < cutoff)
    {
        return Plan {
            snapshots: snapshots.iter().map(|s| s.id).collect(),
            segments: wal.iter().map(|w| w.number).collect(),
        };
    }
    let Some(anchor) = snapshots
        .iter()
        .filter(|s| s.at <= cutoff)
        .max_by_key(|s| (s.at, s.capture, s.id))
    else {
        return Plan::default();
    };
    Plan {
        snapshots: snapshots
            .iter()
            .filter(|s| s.id != anchor.id && s.at < cutoff)
            .map(|s| s.id)
            .collect(),
        segments: wal
            .iter()
            .filter(|w| w.last_at < cutoff && w.last_capture <= anchor.capture)
            .map(|w| w.number)
            .collect(),
    }
}
fn fail(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("Backup retention: {e}"))
}

/// One pass retires/deletes at most 100 objects. The caller serializes this with
/// backup publication. Each remote delete holds the tenant/lease rows, so a
/// reassignment cannot race deletion by its former owner.
pub async fn run(
    db: &TenantDb,
    ledger: &PostgresLedger,
    store: &dyn ObjectStore,
) -> Result<usize, AppError> {
    let mut tx = ledger.pool.begin().await?;
    let current = ledger.lock_owner(db, &mut tx).await?;
    let now: DateTime<Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&mut *tx)
        .await?;
    let cutoff = now - Duration::days(90);
    let generations = sqlx::query("SELECT id,state,sealed_at FROM replication_generations WHERE tenant_id=$1 ORDER BY created_at,id").bind(db.tenant_id()).fetch_all(&mut *tx).await?;
    let mut budget = 100usize;
    // Existing tombstones consume the budget first; failures cannot grow the
    // deletion queue indefinitely while object storage is unavailable.
    let pending:i64=sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM snapshot_manifests WHERE tenant_id=$1 AND retired_at IS NOT NULL AND deleted_at IS NULL)+(SELECT COUNT(*) FROM replication_segments s JOIN replication_generations g ON g.id=s.generation_id WHERE g.tenant_id=$1 AND s.retired_at IS NOT NULL AND s.deleted_at IS NULL)").bind(db.tenant_id()).fetch_one(&mut *tx).await?;
    budget = budget.saturating_sub(pending as usize);
    for generation in generations {
        if budget == 0 {
            break;
        }
        let id: Uuid = generation.get("id");
        let sealed: Option<DateTime<Utc>> = generation.get("sealed_at");
        let expired = Some(id) != current
            && generation.get::<String, _>("state") != "ACTIVE"
            && sealed.is_some_and(|t| t < cutoff);
        let rows=sqlx::query("SELECT id,snapshot_at,capture_number FROM snapshot_manifests WHERE generation_id=$1 AND verified_at IS NOT NULL AND retired_at IS NULL AND source_checksum_sha256 IS NOT NULL AND capture_number IS NOT NULL").bind(id).fetch_all(&mut *tx).await?;
        let snapshots: Vec<Snapshot> = rows
            .into_iter()
            .map(|r| Snapshot {
                id: r.get(0),
                at: r.get(1),
                capture: r.get::<i64, _>(2) as u64,
            })
            .collect();
        let rows=sqlx::query("SELECT segment_number,capture FROM replication_segments WHERE generation_id=$1 AND verified_at IS NOT NULL AND retired_at IS NULL").bind(id).fetch_all(&mut *tx).await?;
        let mut wal = vec![];
        for row in rows {
            let c: super::wal::Capture = serde_json::from_value(row.get(1)).map_err(fail)?;
            let at = crate::time::parse_sqlite_timestamp(
                c.last_captured_at.as_deref().unwrap_or(&c.captured_at),
            )
            .map_err(fail)?;
            wal.push(Wal {
                number: row.get(0),
                last_at: at,
                last_capture: c.range_end(),
            });
        }
        let selected = plan(&snapshots, &wal, cutoff, expired);
        for number in selected.segments.into_iter().take(budget) {
            sqlx::query("UPDATE replication_segments SET retired_at=$3 WHERE generation_id=$1 AND segment_number=$2 AND retired_at IS NULL AND verified_at IS NOT NULL").bind(id).bind(number).bind(now).execute(&mut *tx).await?;
            budget -= 1;
        }
        for snapshot in selected.snapshots.into_iter().take(budget) {
            sqlx::query("UPDATE snapshot_manifests SET retired_at=$2 WHERE id=$1 AND retired_at IS NULL AND verified_at IS NOT NULL").bind(snapshot).bind(now).execute(&mut *tx).await?;
            budget -= 1;
        }
    }
    db.ensure_current_owner()?;
    tx.commit().await?;
    let rows=sqlx::query("SELECT 'snapshot' AS kind,object_key FROM snapshot_manifests WHERE tenant_id=$1 AND retired_at IS NOT NULL AND deleted_at IS NULL UNION ALL SELECT 'wal',s.object_key FROM replication_segments s JOIN replication_generations g ON g.id=s.generation_id WHERE g.tenant_id=$1 AND s.retired_at IS NOT NULL AND s.deleted_at IS NULL LIMIT 100").bind(db.tenant_id()).fetch_all(&ledger.pool).await?;
    let mut deleted = 0;
    for row in rows {
        let key: String = row.get("object_key");
        let table = if row.get::<String, _>("kind") == "snapshot" {
            "snapshot_manifests"
        } else {
            "replication_segments"
        };
        let mut tx = ledger.pool.begin().await?;
        ledger.lock_owner(db, &mut tx).await?;
        // The locked lease has >30s left; cap all remote work to ten seconds.
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let path = Path::from(key.clone());
            match store.delete(&path).await {
                Ok(()) | Err(object_store::Error::NotFound { .. }) => {}
                Err(e) => return Err(fail(e)),
            };
            match store.head(&path).await {
                Err(object_store::Error::NotFound { .. }) => Ok(()),
                Ok(_) => Err(fail("Deleted object remains visible")),
                Err(e) => Err(fail(e)),
            }
        })
        .await
        .map_err(fail)??;
        db.ensure_current_owner()?;
        sqlx::query(&format!("UPDATE {table} SET deleted_at=clock_timestamp() WHERE object_key=$1 AND retired_at IS NOT NULL AND deleted_at IS NULL")).bind(&key).execute(&mut *tx).await?;
        tx.commit().await?;
        deleted += 1;
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snap(days: i64, capture: u64) -> Snapshot {
        Snapshot {
            id: Uuid::new_v4(),
            at: Utc::now() - Duration::days(days),
            capture,
        }
    }
    fn wal(days: i64, last_capture: u64) -> Wal {
        Wal {
            number: last_capture as i64,
            last_at: Utc::now() - Duration::days(days),
            last_capture,
        }
    }
    #[test]
    fn preserves_anchor_and_bridge_to_window_and_straddling_batches() {
        let old = snap(130, 5);
        let anchor = snap(100, 10);
        let recent = snap(20, 40);
        let p = plan(
            &[old.clone(), anchor.clone(), recent.clone()],
            &[wal(120, 5), wal(95, 11), wal(89, 12)],
            Utc::now() - Duration::days(90),
            false,
        );
        assert_eq!(p.snapshots, vec![old.id]);
        assert_eq!(p.segments, vec![5]);
    }
    #[test]
    fn no_anchor_and_idle_current_generation_fail_closed() {
        assert_eq!(
            plan(
                &[snap(20, 5)],
                &[wal(120, 1)],
                Utc::now() - Duration::days(90),
                false
            ),
            Plan::default()
        );
        assert_eq!(
            plan(&[snap(200, 5)], &[], Utc::now() - Duration::days(90), false),
            Plan::default()
        );
    }
    #[test]
    fn sealed_history_expires_only_when_all_captures_age_out() {
        let s = snap(200, 5);
        let cutoff = Utc::now() - Duration::days(90);
        assert_eq!(
            plan(&[s.clone()], &[wal(100, 6)], cutoff, true),
            Plan {
                snapshots: vec![s.id],
                segments: vec![6]
            }
        );
        assert_eq!(plan(&[s], &[wal(80, 6)], cutoff, true), Plan::default());
    }
}

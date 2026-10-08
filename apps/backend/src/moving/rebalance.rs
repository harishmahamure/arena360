//! Measured resource budgets drive placement; in-flight copies reserve capacity.
use super::control;
use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;
pub type Resources = BTreeMap<String, f64>;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Capacity {
    pub hardware_profile: String,
    pub benchmark_id: String,
    pub measured_at: DateTime<Utc>,
    pub headroom: f64,
    pub limits: Resources,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Demand {
    pub tenant_id: Uuid,
    pub resources: Resources,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Measurements {
    pub measured_at: DateTime<Utc>,
    pub cells: Vec<CellOverhead>,
    pub tenants: Vec<Demand>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CellOverhead {
    pub cell_id: Uuid,
    pub resources: Resources,
}
fn retained_storage(demand: &Resources) -> Resources {
    demand
        .iter()
        .filter(|(name, _)| {
            matches!(
                name.as_str(),
                "disk_bytes"
                    | "sqlite_bytes"
                    | "duckdb_bytes"
                    | "wal_bytes"
                    | "spool_bytes"
                    | "temporary_bytes"
            )
        })
        .map(|(k, n)| (k.clone(), *n))
        .collect()
}
#[derive(Clone, Debug)]
pub struct Cell {
    pub id: Uuid,
    pub state: String,
    pub capacity: Capacity,
}
#[derive(Clone, Debug)]
pub struct Tenant {
    pub id: Uuid,
    pub owner: Uuid,
    pub movable: bool,
}
#[derive(Debug, Serialize)]
pub struct Proposal {
    pub tenant_id: Uuid,
    pub source_cell: Uuid,
    pub target_cell: Uuid,
    pub move_id: Option<Uuid>,
}
#[derive(Debug, Serialize)]
pub struct Plan {
    pub applied: bool,
    pub moves: Vec<Proposal>,
    pub projected_utilization: BTreeMap<Uuid, f64>,
    pub remaining_pressure: Vec<Uuid>,
}
fn invalid(s: impl Into<String>) -> AppError {
    AppError::BadRequest(s.into())
}
fn add(load: &mut Resources, demand: &Resources, sign: f64) -> Result<(), AppError> {
    for (key, n) in demand {
        let value = load.entry(key.clone()).or_default();
        *value += sign * n;
        if !value.is_finite() {
            return Err(invalid("Capacity sum overflow"));
        }
        if *value < 0.0 && *value > -1e-8 {
            *value = 0.0;
        }
        if *value < 0.0 {
            return Err(invalid("Capacity subtraction underflow"));
        }
    }
    Ok(())
}
fn pressure(cell: &Cell, load: &Resources) -> f64 {
    cell.capacity
        .limits
        .iter()
        .map(|(key, limit)| {
            load.get(key).copied().unwrap_or_default() / (limit * cell.capacity.headroom)
        })
        .fold(0.0, f64::max)
}
/// Deterministic preview. Reservations contain only pre-handoff target copies.
pub fn plan(
    cells: &[Cell],
    tenants: &[Tenant],
    reservations: &[(Uuid, Uuid)],
    measurements: &Measurements,
    now: DateTime<Utc>,
    drain: Option<Uuid>,
    maximum: usize,
) -> Result<Plan, AppError> {
    if maximum == 0 || maximum > 100 {
        return Err(invalid("Maximum moves must be 1..100"));
    }
    if measurements.measured_at > now + chrono::Duration::seconds(30)
        || measurements.measured_at < now - chrono::Duration::minutes(5)
    {
        return Err(invalid(
            "Tenant measurements must be at most five minutes old",
        ));
    }
    let by_cell: BTreeMap<_, _> = cells.iter().map(|c| (c.id, c)).collect();
    if by_cell.len() != cells.len() || cells.is_empty() {
        return Err(invalid("Cells must be nonempty and unique"));
    }
    if drain.is_some_and(|id| !by_cell.contains_key(&id)) {
        return Err(invalid("Drain cell is unknown"));
    }
    let dimensions: BTreeSet<_> = cells[0].capacity.limits.keys().cloned().collect();
    if dimensions.len() < 2 {
        return Err(invalid(
            "Capacity requires at least two measured resource dimensions",
        ));
    }
    for cell in cells {
        let c = &cell.capacity;
        if c.hardware_profile.trim().is_empty()
            || c.benchmark_id.trim().is_empty()
            || c.measured_at > now + chrono::Duration::seconds(30)
            || !c.headroom.is_finite()
            || c.headroom <= 0.0
            || c.headroom > 1.0
            || c.limits.keys().cloned().collect::<BTreeSet<_>>() != dimensions
            || c.limits
                .values()
                .any(|n| !n.is_finite() || *n <= 0.0 || !(*n * c.headroom).is_normal())
        {
            return Err(invalid(format!(
                "Cell {} has invalid measured capacity",
                cell.id
            )));
        }
        if !matches!(cell.state.as_str(), "ACTIVE" | "DRAINING" | "OFFLINE") {
            return Err(invalid("Unknown cell state"));
        }
    }
    let demands: BTreeMap<_, _> = measurements
        .tenants
        .iter()
        .map(|d| (d.tenant_id, &d.resources))
        .collect();
    let tenant_ids: BTreeSet<_> = tenants.iter().map(|t| t.id).collect();
    if demands.len() != measurements.tenants.len()
        || tenant_ids.len() != tenants.len()
        || demands.keys().copied().collect::<BTreeSet<_>>() != tenant_ids
    {
        return Err(invalid(
            "Measurements must cover every assigned tenant exactly once",
        ));
    }
    for demand in demands.values() {
        if demand.keys().cloned().collect::<BTreeSet<_>>() != dimensions
            || demand.values().any(|n| !n.is_finite() || *n < 0.0)
        {
            return Err(invalid("Tenant resource dimensions or values are invalid"));
        }
    }
    let overhead: BTreeMap<_, _> = measurements
        .cells
        .iter()
        .map(|c| (c.cell_id, &c.resources))
        .collect();
    if overhead.len() != measurements.cells.len()
        || overhead.keys().copied().collect::<BTreeSet<_>>()
            != by_cell.keys().copied().collect::<BTreeSet<_>>()
    {
        return Err(invalid(
            "Measured overhead must cover every cell exactly once",
        ));
    }
    for demand in overhead.values() {
        if demand.keys().cloned().collect::<BTreeSet<_>>() != dimensions
            || demand.values().any(|n| !n.is_finite() || *n < 0.0)
        {
            return Err(invalid("Cell overhead dimensions or values are invalid"));
        }
    }
    let mut loads: BTreeMap<Uuid, Resources> = overhead
        .into_iter()
        .map(|(id, load)| (id, load.clone()))
        .collect();
    for tenant in tenants {
        add(
            loads
                .get_mut(&tenant.owner)
                .ok_or_else(|| invalid("Tenant owner is unknown"))?,
            demands[&tenant.id],
            1.0,
        )?;
    }
    let mut reserved = BTreeSet::new();
    for (tenant, target) in reservations {
        let current = tenants
            .iter()
            .find(|t| t.id == *tenant)
            .ok_or_else(|| invalid("Reservation tenant is unknown"))?;
        if current.owner == *target || !reserved.insert(*tenant) {
            return Err(invalid("Invalid duplicate or owner reservation"));
        }
        add(
            loads
                .get_mut(target)
                .ok_or_else(|| invalid("Reservation cell is unknown"))?,
            demands[tenant],
            1.0,
        )?;
    }
    let draining = |c: &Cell| c.state == "DRAINING" || drain == Some(c.id);
    let mut selected = BTreeSet::new();
    let mut moves = Vec::new();
    for _ in 0..maximum {
        // Prefer draining cells, then the highest source utilization. UUIDs
        // break ties so repeated previews are stable.
        let mut best: Option<(bool, f64, f64, Uuid, Uuid, Uuid)> = None;
        for tenant in tenants {
            if !tenant.movable || selected.contains(&tenant.id) || reserved.contains(&tenant.id) {
                continue;
            }
            let source = by_cell[&tenant.owner];
            if source.state == "OFFLINE" {
                continue;
            }
            let source_pressure = pressure(source, &loads[&source.id]);
            if !draining(source) && source_pressure <= 1.0 {
                continue;
            }
            for target in cells {
                if target.id == source.id || target.state != "ACTIVE" || draining(target) {
                    continue;
                }
                let mut after = loads[&target.id].clone();
                add(&mut after, demands[&tenant.id], 1.0)?;
                let target_pressure = pressure(target, &after);
                if target_pressure > 1.0
                    || (!draining(source) && target_pressure + 0.01 >= source_pressure)
                {
                    continue;
                }
                let candidate = (
                    draining(source),
                    source_pressure,
                    -target_pressure,
                    tenant.id,
                    source.id,
                    target.id,
                );
                if best.as_ref().is_none_or(|b| candidate > b.clone()) {
                    best = Some(candidate);
                }
            }
        }
        let Some((_, _, _, tenant, source, target)) = best else {
            break;
        };
        add(loads.get_mut(&source).unwrap(), demands[&tenant], -1.0)?;
        // Source storage is retained for seven days, even after handoff.
        add(
            loads.get_mut(&source).unwrap(),
            &retained_storage(demands[&tenant]),
            1.0,
        )?;
        add(loads.get_mut(&target).unwrap(), demands[&tenant], 1.0)?;
        selected.insert(tenant);
        moves.push(Proposal {
            tenant_id: tenant,
            source_cell: source,
            target_cell: target,
            move_id: None,
        });
    }
    let projected_utilization: BTreeMap<_, _> = cells
        .iter()
        .map(|c| (c.id, pressure(c, &loads[&c.id])))
        .collect();
    if projected_utilization.values().any(|p| !p.is_finite()) {
        return Err(invalid("Normalized utilization overflow"));
    }
    let remaining_pressure = cells
        .iter()
        .filter(|c| {
            projected_utilization[&c.id] > 1.0
                || (draining(c)
                    && tenants
                        .iter()
                        .any(|t| t.owner == c.id && !selected.contains(&t.id)))
        })
        .map(|c| c.id)
        .collect();
    Ok(Plan {
        applied: false,
        moves,
        projected_utilization,
        remaining_pressure,
    })
}
/// Apply reruns placement while locking cell budgets and tenant ownership.
/// It queues explicit moves; cells carry out the API-0060 state machine.
pub async fn run(
    pool: &PgPool,
    measurements: &Measurements,
    drain: Option<Uuid>,
    maximum: usize,
    apply: bool,
) -> Result<Plan, AppError> {
    let mut tx = pool.begin().await?;
    if !apply {
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await?;
    }
    if apply {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('arena360-rebalance',0))")
            .execute(&mut *tx)
            .await?;
    }
    let cell_sql = if apply {
        "SELECT id,state,capacity_weights FROM cells ORDER BY id FOR UPDATE"
    } else {
        "SELECT id,state,capacity_weights FROM cells ORDER BY id"
    };
    let mut cells = Vec::new();
    for row in sqlx::query(cell_sql).fetch_all(&mut *tx).await? {
        let id: Uuid = row.get(0);
        let capacity: Capacity = serde_json::from_value(row.get(2)).map_err(|e| {
            invalid(format!(
                "Cell {id} requires benchmark capacity_weights: {e}"
            ))
        })?;
        cells.push(Cell {
            id,
            state: row.get(1),
            capacity,
        });
    }
    // Lock in a separate query, before reading leases, to get the current
    // ownership and expiry after a concurrent handoff/renewal finishes.
    if apply {
        sqlx::query("SELECT id FROM tenants WHERE owner_cell IS NOT NULL ORDER BY id FOR UPDATE")
            .fetch_all(&mut *tx)
            .await?;
    }
    let rows=sqlx::query("SELECT t.id,t.owner_cell,t.state='ACTIVE' AND COALESCE(l.owner_cell=t.owner_cell AND l.ownership_generation=t.ownership_generation AND l.expires_at>clock_timestamp()+INTERVAL '30 seconds',false) AS movable FROM tenants t LEFT JOIN tenant_leases l ON l.tenant_id=t.id WHERE t.owner_cell IS NOT NULL AND t.state NOT IN ('COLD','DELETED','FAILED') ORDER BY t.id").fetch_all(&mut *tx).await?;
    let tenants: Vec<_> = rows
        .into_iter()
        .map(|r| Tenant {
            id: r.get(0),
            owner: r.get(1),
            movable: r.get(2),
        })
        .collect();
    let reservations:Vec<(Uuid,Uuid)>=sqlx::query_as("SELECT tenant_id,target_cell FROM tenant_moves WHERE phase IN ('PREPARING_MOVE','COPYING','CUTOVER') ORDER BY id").fetch_all(&mut *tx).await?;
    let now: DateTime<Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&mut *tx)
        .await?;
    let mut plan = plan(
        &cells,
        &tenants,
        &reservations,
        measurements,
        now,
        drain,
        maximum,
    )?;
    if apply {
        if let Some(id) = drain {
            let cell = cells.iter().find(|c| c.id == id).unwrap();
            if cell.state == "OFFLINE" {
                return Err(invalid("An offline cell requires recovery, not draining"));
            }
            sqlx::query(
                "UPDATE cells SET state='DRAINING',updated_at=clock_timestamp() WHERE id=$1",
            )
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        for proposed in &mut plan.moves {
            proposed.move_id = Some(
                control::enqueue_locked(&mut tx, proposed.tenant_id, proposed.target_cell).await?,
            );
        }
        plan.applied = true;
        tx.commit().await?;
    } else {
        tx.rollback().await?;
    }
    Ok(plan)
}
/// Retire only an empty draining cell. Row locking blocks new acquisitions.
pub async fn decommission(pool: &PgPool, cell: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    let state: Option<String> =
        sqlx::query_scalar("SELECT state FROM cells WHERE id=$1 FOR UPDATE")
            .bind(cell)
            .fetch_optional(&mut *tx)
            .await?;
    if state.as_deref() != Some("DRAINING") {
        return Err(invalid("Only a draining cell can be decommissioned"));
    }
    let occupied:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenants WHERE owner_cell=$1) OR EXISTS(SELECT 1 FROM tenant_moves WHERE target_cell=$1 AND phase NOT IN ('ACTIVE','CANCELLED')) OR EXISTS(SELECT 1 FROM tenant_leases WHERE owner_cell=$1 AND expires_at>clock_timestamp())").bind(cell).fetch_one(&mut *tx).await?;
    if occupied {
        return Err(AppError::Conflict(
            "Cell still owns tenants, live leases, or incoming moves".into(),
        ));
    }
    sqlx::query("UPDATE cells SET state='OFFLINE',updated_at=clock_timestamp() WHERE id=$1")
        .bind(cell)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

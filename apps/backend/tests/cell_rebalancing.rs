use chrono::Utc;
use gaming_cafe_api::moving::rebalance::{
    self, Capacity, Cell, CellOverhead, Demand, Measurements, Resources, Tenant,
};
use uuid::Uuid;
fn resources(n: f64) -> Resources {
    [("write_tps".into(), n), ("disk_bytes".into(), n * 1000.0)].into()
}
fn capacity(n: f64) -> Capacity {
    Capacity {
        hardware_profile: format!("fixture-{n}"),
        benchmark_id: "fixture-only".into(),
        measured_at: Utc::now(),
        headroom: 1.0,
        limits: resources(n),
    }
}
fn fixture() -> (Vec<Cell>, Vec<Tenant>, Measurements) {
    let a = Uuid::from_u128(1);
    let b = Uuid::from_u128(2);
    let cells = vec![
        Cell {
            id: a,
            state: "ACTIVE".into(),
            capacity: capacity(10.0),
        },
        Cell {
            id: b,
            state: "ACTIVE".into(),
            capacity: capacity(10.0),
        },
    ];
    let tenants = vec![
        Tenant {
            id: Uuid::from_u128(11),
            owner: a,
            movable: true,
        },
        Tenant {
            id: Uuid::from_u128(12),
            owner: a,
            movable: true,
        },
    ];
    let measurements = Measurements {
        measured_at: Utc::now(),
        cells: cells
            .iter()
            .map(|c| CellOverhead {
                cell_id: c.id,
                resources: resources(0.0),
            })
            .collect(),
        tenants: tenants
            .iter()
            .map(|t| Demand {
                tenant_id: t.id,
                resources: resources(6.0),
            })
            .collect(),
    };
    (cells, tenants, measurements)
}
#[test]
fn weighted_placement_respects_smallest_resource_and_pending_copy_capacity() {
    let (mut cells, tenants, measurements) = fixture();
    let initial =
        rebalance::plan(&cells, &tenants, &[], &measurements, Utc::now(), None, 10).unwrap();
    assert_eq!(initial.moves.len(), 1);
    assert_eq!(initial.projected_utilization[&cells[0].id], 1.2);
    assert_eq!(initial.projected_utilization[&cells[1].id], 0.6);
    let pending = [(initial.moves[0].tenant_id, cells[1].id)];
    assert!(rebalance::plan(
        &cells,
        &tenants,
        &pending,
        &measurements,
        Utc::now(),
        None,
        10
    )
    .unwrap()
    .moves
    .is_empty());
    cells[1].capacity.limits.insert("disk_bytes".into(), 5000.0);
    assert!(
        rebalance::plan(&cells, &tenants, &[], &measurements, Utc::now(), None, 10)
            .unwrap()
            .moves
            .is_empty()
    );
    cells[1].capacity = capacity(30.0);
    let draining = rebalance::plan(
        &cells,
        &tenants,
        &[],
        &measurements,
        Utc::now(),
        Some(cells[0].id),
        10,
    )
    .unwrap();
    assert_eq!(draining.moves.len(), 2);
    assert!(
        draining.remaining_pressure.contains(&cells[0].id),
        "Retained source storage remains charged"
    );
    let mut overhead = measurements.clone();
    overhead.cells[1].resources = resources(29.0);
    assert!(rebalance::plan(
        &cells,
        &tenants,
        &[],
        &overhead,
        Utc::now(),
        Some(cells[0].id),
        10
    )
    .unwrap()
    .moves
    .is_empty());
}
#[test]
fn rejects_missing_stale_duplicate_and_nonfinite_measurements() {
    let (cells, tenants, mut m) = fixture();
    m.measured_at -= chrono::Duration::minutes(6);
    assert!(rebalance::plan(&cells, &tenants, &[], &m, Utc::now(), None, 10).is_err());
    m.measured_at = Utc::now();
    m.tenants.pop();
    assert!(rebalance::plan(&cells, &tenants, &[], &m, Utc::now(), None, 10).is_err());
    m.tenants.push(m.tenants[0].clone());
    assert!(rebalance::plan(&cells, &tenants, &[], &m, Utc::now(), None, 10).is_err());
    let (_, _, mut m) = fixture();
    m.tenants[0].resources.insert("write_tps".into(), f64::NAN);
    assert!(rebalance::plan(&cells, &tenants, &[], &m, Utc::now(), None, 10).is_err());
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn preview_apply_reserve_drain_and_decommission() {
    use sqlx::ConnectOptions;
    let url = std::env::var("CONTROL_TEST_DATABASE_URL").unwrap();
    let admin = sqlx::PgPool::connect(&url).await.unwrap();
    let schema = format!("rebalance_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let options = url
        .parse::<sqlx::postgres::PgConnectOptions>()
        .unwrap()
        .options([("search_path", schema.as_str())])
        .disable_statement_logging();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    let (cells, tenants, mut m) = fixture();
    for c in &cells {
        sqlx::query("INSERT INTO cells(id,name,address,capacity_weights) VALUES($1,$2,$3,$4)")
            .bind(c.id)
            .bind(c.id.to_string())
            .bind(format!("http://{}.invalid", c.id))
            .bind(serde_json::to_value(&c.capacity).unwrap())
            .execute(&pool)
            .await
            .unwrap();
    }
    for t in &tenants {
        sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,ownership_generation,state) VALUES($1,$2,'Rebalance','UTC',$3,1,'ACTIVE')").bind(t.id).bind(t.id.to_string()).bind(t.owner).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO tenant_leases(tenant_id,owner_cell,ownership_generation,expires_at) VALUES($1,$2,1,clock_timestamp()+INTERVAL '5 minutes')").bind(t.id).bind(t.owner).execute(&pool).await.unwrap();
    }
    let preview = rebalance::run(&pool, &m, None, 10, false).await.unwrap();
    assert_eq!(preview.moves.len(), 1);
    assert!(!preview.applied);
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM tenant_moves")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pending, 0);
    let (a, b) = tokio::join!(
        rebalance::run(&pool, &m, None, 10, true),
        rebalance::run(&pool, &m, None, 10, true)
    );
    assert_eq!(a.unwrap().moves.len() + b.unwrap().moves.len(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tenant_moves")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    let blocked = rebalance::run(&pool, &m, Some(cells[0].id), 10, true)
        .await
        .unwrap();
    assert!(blocked.moves.is_empty());
    assert!(blocked.remaining_pressure.contains(&cells[0].id));
    assert!(rebalance::decommission(&pool, cells[0].id).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM cells WHERE id=$1")
            .bind(cells[0].id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "DRAINING"
    );
    // A failed apply must roll back a drain state transition too.
    m.tenants.pop();
    assert!(rebalance::run(&pool, &m, Some(cells[1].id), 10, true)
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM cells WHERE id=$1")
            .bind(cells[1].id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "ACTIVE"
    );
    sqlx::query("DELETE FROM tenants")
        .execute(&pool)
        .await
        .unwrap();
    rebalance::decommission(&pool, cells[0].id).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM cells WHERE id=$1")
            .bind(cells[0].id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "OFFLINE"
    );
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

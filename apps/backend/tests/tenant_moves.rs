use gaming_cafe_api::{
    control::{LeaseClient, LeaseConfig},
    metrics::Metrics,
    moving::{agent::Agent, control, source, target},
    replication::{
        crypto::TenantKeys, ledger::PostgresLedger, recovery::Recoverer, wal, worker::Worker,
    },
    tenancy::{tenant_path, TenantDbConfig, TenantDbManager},
};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
async fn write(db: &gaming_cafe_api::tenancy::TenantDb, n: i64) {
    db.with_writer(move |c| {
        Box::pin(async move {
            sqlx::query("INSERT INTO moved_facts VALUES(?)")
                .bind(n)
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires an isolated control-plane database"]
async fn verified_move_catches_up_fences_source_and_resumes_completion() {
    use sqlx::{sqlite::SqliteConnectOptions, Connection};
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(12)
        .connect(&std::env::var("CONTROL_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    let root = std::env::temp_dir().join(format!("arena360-move-{}", Uuid::new_v4()));
    let tenant = Uuid::new_v4();
    let old = Uuid::new_v4();
    let new = Uuid::new_v4();
    for cell in [old, new] {
        sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)")
            .bind(cell)
            .bind(format!("move-{cell}"))
            .bind(format!("http://{cell}.invalid"))
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,ownership_generation,state) VALUES($1,$2,'Move','UTC',$3,1,'ACTIVE')").bind(tenant).bind(format!("move-{tenant}")).bind(old).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO subscriptions(tenant_id,plan_code,status,starts_at,ends_at) VALUES($1,'trial','TRIAL',NOW(),NOW()+INTERVAL '30 days')").bind(tenant).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tenant_leases(tenant_id,owner_cell,ownership_generation,expires_at) VALUES($1,$2,1,NOW()+INTERVAL '5 minutes')").bind(tenant).bind(old).execute(&pool).await.unwrap();
    let old_root = root.join("old");
    let path = tenant_path(&old_root, tenant);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut connection = sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE moved_facts(number INTEGER PRIMARY KEY)")
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
    let schema_pool = sqlx::SqlitePool::connect_with(SqliteConnectOptions::new().filename(&path))
        .await
        .unwrap();
    gaming_cafe_api::tenancy::migrate(&schema_pool)
        .await
        .unwrap();
    schema_pool.close().await;
    sqlx::query("UPDATE tenants SET schema_version=$2 WHERE id=$1")
        .bind(tenant)
        .bind(gaming_cafe_api::tenancy::target_schema_version())
        .execute(&pool)
        .await
        .unwrap();
    let keys = TenantKeys::new(root.join("keys"));
    wal::durable_create(&keys.path(tenant), &[42u8; 32]).unwrap();
    let store = Arc::new(object_store::memory::InMemory::new());
    let mut contexts = vec![];
    let mut agents = vec![];
    for (cell, dir) in [(old, old_root.clone()), (new, root.join("new"))] {
        let leases =
            Arc::new(LeaseClient::new(pool.clone(), cell, LeaseConfig::default()).unwrap());
        if cell == old {
            leases.acquire_assigned(tenant).await.unwrap();
        }
        let manager = Arc::new(
            TenantDbManager::new(
                TenantDbConfig {
                    root: dir.clone(),
                    ..Default::default()
                },
                leases.clone(),
            )
            .unwrap()
            .with_background_jobs(
                gaming_cafe_api::background::BackgroundJobs::new(
                    gaming_cafe_api::background::Limits {
                        outbox_slots: 1,
                        background_slots: 1,
                        backfill_slots: 1,
                    },
                )
                .unwrap(),
            ),
        );
        let ledger = Arc::new(PostgresLedger {
            pool: pool.clone(),
            cell_id: cell,
        });
        let worker = Arc::new(Worker {
            gates: Default::default(),
            store: store.clone(),
            ledger: ledger.clone(),
            keys: keys.clone(),
            metrics: Arc::new(Metrics::default()),
        });
        let context = Arc::new(Recoverer {
            ledger,
            leases,
            databases: manager,
            store: store.clone(),
            keys: keys.clone(),
            staging_root: dir.join("move-staging"),
            analytics: None,
        });
        agents.push(Agent {
            recovery: context.clone(),
            worker,
        });
        contexts.push(context);
    }
    let db = contexts[0].databases.open(tenant).await.unwrap();
    write(&db, 1).await;
    let requested = control::enqueue(&pool, tenant, new).await.unwrap();
    assert_eq!(requested.phase, "PREPARING_MOVE");
    assert!(control::enqueue(&pool, tenant, new).await.is_err());
    agents[0].advance(requested.id).await.unwrap();
    assert_eq!(
        control::get(&pool, requested.id).await.unwrap().phase,
        "COPYING"
    );
    sqlx::query("UPDATE tenants SET schema_version=0 WHERE id=$1")
        .bind(tenant)
        .execute(&pool)
        .await
        .unwrap();
    assert!(agents[1].advance(requested.id).await.is_err());
    sqlx::query("UPDATE tenants SET schema_version=$2 WHERE id=$1")
        .bind(tenant)
        .bind(gaming_cafe_api::tenancy::target_schema_version())
        .execute(&pool)
        .await
        .unwrap();
    agents[1].advance(requested.id).await.unwrap();
    sqlx::query("UPDATE tenant_moves SET target_prepared_at=clock_timestamp()-INTERVAL '11 seconds' WHERE id=$1").bind(requested.id).execute(&pool).await.unwrap();
    assert!(source::cutover(
        &pool,
        db.clone(),
        agents[0].worker.clone(),
        contexts[0].leases.clone(),
        requested.id
    )
    .await
    .is_err());
    assert_eq!(
        control::get(&pool, requested.id).await.unwrap().phase,
        "COPYING"
    );
    agents[1].advance(requested.id).await.unwrap();
    // Writes continue after pre-copy; the final capture must contain them too.
    write(&db, 2).await;
    write(&db, 3).await;
    let source_pool = pool.clone();
    let source_db = db.clone();
    let worker = agents[0].worker.clone();
    let leases = contexts[0].leases.clone();
    let id = requested.id;
    let handoff =
        tokio::spawn(
            async move { source::cutover(&source_pool, source_db, worker, leases, id).await },
        );
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if control::get(&pool, id).await.unwrap().phase == "CUTOVER" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(control::cancel(&pool, id).await.is_err());
    agents[1].advance(id).await.unwrap();
    let grant = handoff.await.unwrap().unwrap();
    assert_eq!(grant.owner_cell, new);
    assert_eq!(grant.ownership_generation, 2);
    assert!(db
        .with_writer(|_| Box::pin(async { Ok(()) }))
        .await
        .is_err());
    assert!(contexts[1].databases.open(tenant).await.is_err());
    let route = gaming_cafe_api::routing::RoutingCache::new(pool.clone());
    assert_eq!(
        route
            .refresh_tenant(tenant)
            .await
            .unwrap()
            .unwrap()
            .owner_cell,
        new
    );
    target::activate(contexts[1].clone(), id).await.unwrap();
    let moved = contexts[1].databases.open(tenant).await.unwrap();
    let facts: Vec<i64> = sqlx::query_scalar("SELECT number FROM moved_facts ORDER BY number")
        .fetch_all(&moved.read_pool().unwrap())
        .await
        .unwrap();
    assert_eq!(facts, [1, 2, 3]);
    write(&moved, 4).await;
    assert!(
        path.is_file(),
        "Old copy must remain available after handoff"
    );
    let done = control::get(&pool, id).await.unwrap();
    assert_eq!(done.phase, "ACTIVE");
    assert!(done.write_gate_milliseconds.unwrap() < 5000);
    // Simulate COMMIT succeeding but the process dying before marker removal.
    moved.close().await.unwrap();
    let new_root = root.join("new");
    let marker = tenant_path(&new_root, tenant)
        .parent()
        .unwrap()
        .join("replication/move-pending.json");
    wal::durable_create(
        &marker,
        &serde_json::to_vec(&serde_json::json!({"move_id":id,"ownership_generation":2})).unwrap(),
    )
    .unwrap();
    let fresh_leases =
        Arc::new(LeaseClient::new(pool.clone(), new, LeaseConfig::default()).unwrap());
    let fresh_databases = Arc::new(
        TenantDbManager::new(
            TenantDbConfig {
                root: new_root.clone(),
                ..Default::default()
            },
            fresh_leases.clone(),
        )
        .unwrap()
        .with_background_jobs(
            gaming_cafe_api::background::BackgroundJobs::new(Default::default()).unwrap(),
        ),
    );
    contexts[1] = Arc::new(Recoverer {
        ledger: contexts[1].ledger.clone(),
        leases: fresh_leases,
        databases: fresh_databases,
        store: store.clone(),
        keys: keys.clone(),
        staging_root: new_root.join("move-staging"),
        analytics: None,
    });
    agents[1].recovery = contexts[1].clone();
    assert!(contexts[1].databases.open(tenant).await.is_err());
    agents[1].resume_completed().await.unwrap();
    assert!(!marker.exists());
    let moved = contexts[1].databases.open(tenant).await.unwrap();
    // A retry cannot transfer ownership again or discard new target writes.
    target::activate(contexts[1].clone(), id).await.unwrap();
    let facts: Vec<i64> = sqlx::query_scalar("SELECT number FROM moved_facts ORDER BY number")
        .fetch_all(&moved.read_pool().unwrap())
        .await
        .unwrap();
    assert_eq!(facts, [1, 2, 3, 4]);
    println!(
        "Move write gate: {}ms",
        done.write_gate_milliseconds.unwrap()
    );
    // A missing target acknowledgement releases the gate and preserves writes.
    let reverse = control::enqueue(&pool, tenant, old).await.unwrap();
    agents[1].advance(reverse.id).await.unwrap();
    agents[0].advance(reverse.id).await.unwrap();
    write(&moved, 5).await;
    let failed = source::cutover(
        &pool,
        moved.clone(),
        agents[1].worker.clone(),
        contexts[1].leases.clone(),
        reverse.id,
    )
    .await;
    assert!(failed.is_err());
    let retry = control::get(&pool, reverse.id).await.unwrap();
    assert_eq!(retry.phase, "COPYING");
    assert!(retry.target_prepared_at.is_none());
    let owner: Uuid = sqlx::query_scalar("SELECT owner_cell FROM tenants WHERE id=$1")
        .bind(tenant)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(owner, new);
    write(&moved, 6).await;
    control::cancel(&pool, reverse.id).await.unwrap();
    assert_eq!(
        control::get(&pool, reverse.id).await.unwrap().phase,
        "CANCELLED"
    );
    agents[0].cleanup().await.unwrap();
    assert!(
        path.is_file(),
        "Retention must preserve the old copy for seven days"
    );
    sqlx::query("UPDATE tenant_moves SET retain_source_until=clock_timestamp()-INTERVAL '1 second' WHERE id=$1").bind(id).execute(&pool).await.unwrap();
    agents[0].cleanup().await.unwrap();
    assert!(!path.exists());
    assert!(tenant_path(&root.join("new"), tenant).is_file());
    write(&moved, 7).await;
    moved.close().await.unwrap();
    db.close().await.unwrap();
    sqlx::query("DELETE FROM tenants WHERE id=$1")
        .bind(tenant)
        .execute(&pool)
        .await
        .unwrap();
    for cell in [old, new] {
        sqlx::query("DELETE FROM cells WHERE id=$1")
            .bind(cell)
            .execute(&pool)
            .await
            .unwrap();
    }
    pool.close().await;
    std::fs::remove_dir_all(root).unwrap();
}

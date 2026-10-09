use gaming_cafe_api::{
    cold::{self as control, Agent},
    control::{LeaseClient, LeaseConfig},
    metrics::Metrics,
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
async fn verified_cold_roundtrip_fences_source_and_handles_concurrent_wake() {
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

    let recent = control::enqueue(&pool, tenant, 86400).await.unwrap();
    assert!(agents[0].cool(recent.id).await.is_err());
    assert!(path.exists());
    control::cancel(&pool, recent.id).await.unwrap();
    let requested = control::enqueue(&pool, tenant, 0).await.unwrap();
    // Missing encryption material must never release or remove the live tenant.
    let key_path = keys.path(tenant);
    std::fs::remove_file(&key_path).unwrap();
    assert!(agents[0].cool(requested.id).await.is_err());
    assert!(path.exists());
    write(&db, 2).await;
    wal::durable_create(&key_path, &[42u8; 32]).unwrap();
    tokio::time::timeout(Duration::from_secs(30), agents[0].cool(requested.id))
        .await
        .unwrap()
        .unwrap();
    assert!(!path.parent().unwrap().exists());
    assert!(key_path.exists());
    let job = control::get(&pool, requested.id).await.unwrap();
    assert_eq!(job.phase, "RELEASED");
    assert!(job.source_cleaned_at.is_some());
    let (state, owner): (String, Option<Uuid>) =
        sqlx::query_as("SELECT state,owner_cell FROM tenants WHERE id=$1")
            .bind(tenant)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, "COLD");
    assert_eq!(owner, None);
    assert!(db
        .with_writer(|c| Box::pin(async move {
            sqlx::query("INSERT INTO moved_facts VALUES(3)")
                .execute(c)
                .await?;
            Ok(())
        }))
        .await
        .is_err());
    sqlx::query("UPDATE cells SET state='DRAINING' WHERE id=$1")
        .bind(old)
        .execute(&pool)
        .await
        .unwrap();
    agents[1].heartbeat().await.unwrap();
    let cache = Arc::new(gaming_cafe_api::routing::RoutingCache::new(pool.clone()));
    let coordinator = control::Coordinator {
        pool: pool.clone(),
        wait_limit: Duration::from_secs(30),
    };
    let router = gaming_cafe_api::routing::TenantRouter::new(cache, None)
        .unwrap()
        .with_cold(coordinator.clone());
    let waking = tokio::spawn(async move { router.remote_address(tenant).await });
    let second = tokio::spawn(async move { coordinator.wake(tenant).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let state: String = sqlx::query_scalar("SELECT state FROM tenants WHERE id=$1")
                .bind(tenant)
                .fetch_one(&pool)
                .await
                .unwrap();
            if state == "RESTORING" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    agents[1].hydrate(tenant).await.unwrap();
    assert_eq!(
        waking.await.unwrap().unwrap(),
        Some(format!("http://{new}.invalid"))
    );
    second.await.unwrap().unwrap();
    // A request that observed COLD before another wake completed cannot reset ACTIVE.
    assert!(gaming_cafe_api::control::LeaseRepository::new(pool.clone())
        .acquire_for_cold(tenant, new, LeaseConfig::default())
        .await
        .is_err());
    let (state, generation): (String, i64) =
        sqlx::query_as("SELECT state,ownership_generation FROM tenants WHERE id=$1")
            .bind(tenant)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, "ACTIVE");
    assert_eq!(generation, 2);
    let restored = contexts[1].databases.open(tenant).await.unwrap();
    let values: Vec<i64> = sqlx::query_scalar("SELECT number FROM moved_facts ORDER BY number")
        .fetch_all(&restored.read_pool().unwrap())
        .await
        .unwrap();
    assert_eq!(values, vec![1, 2]);
    write(&restored, 3).await;
    let job = control::get(&pool, requested.id).await.unwrap();
    assert_eq!(job.phase, "ACTIVE");
    println!(
        "cold hydration operations RTO: {} ms",
        job.hydration_milliseconds.unwrap()
    );
    // Simulate recovery winning the crash window before assignment bookkeeping.
    sqlx::query("UPDATE tenant_cold_jobs SET phase='RELEASED',target_cell=NULL,target_ownership_generation=NULL,hydration_requested_at=NULL,operations_ready_at=NULL,hydration_milliseconds=NULL WHERE id=$1").bind(requested.id).execute(&pool).await.unwrap();
    agents[1].tick().await.unwrap();
    assert_eq!(
        control::get(&pool, requested.id).await.unwrap().phase,
        "ACTIVE"
    );
    agents[0].cleanup(requested.id).await.unwrap();
    assert!(restored.path().exists());

    std::fs::remove_dir_all(root).unwrap();
}

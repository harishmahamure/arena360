use gaming_cafe_api::{
    control::LeaseClient,
    tenancy::{
        rollout::{self, State},
        target_schema_version, tenant_path, MigrationContext, MigrationHook, MigrationOrchestrator,
        MigrationState, TenantDbConfig, TenantDbManager,
    },
};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;
struct FailOnce(std::sync::atomic::AtomicBool);

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn all_tenants_migrate_on_their_owning_cells_and_repeated_runs_reuse_work() {
    use sqlx::{ConnectOptions, Connection};
    let url = std::env::var("CONTROL_TEST_DATABASE_URL").unwrap();
    let admin = PgPool::connect(&url).await.unwrap();
    let schema = format!("rollout_all_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(12)
        .connect_with(
            url.parse::<sqlx::postgres::PgConnectOptions>()
                .unwrap()
                .options([("search_path", schema.as_str())])
                .disable_statement_logging(),
        )
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    assert_eq!(rollout::create_all(&pool).await.unwrap(), None);
    let root = std::env::temp_dir().join(format!("arena360-rollout-all-{}", Uuid::new_v4()));
    let mut tenants = Vec::new();
    let mut managers = Vec::new();
    let mut orchestrators = Vec::new();
    let target = target_schema_version();
    for index in 0..2 {
        let cell = Uuid::new_v4();
        sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)")
            .bind(cell)
            .bind(format!("All {index}"))
            .bind(format!("http://cell-{index}.invalid"))
            .execute(&pool)
            .await
            .unwrap();
        let leases = Arc::new(LeaseClient::new(pool.clone(), cell, Default::default()).unwrap());
        for n in 0..2 {
            let tenant = Uuid::new_v4();
            sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,ownership_generation,state) VALUES($1,$2,'All','UTC',$3,1,'ACTIVE')")
                .bind(tenant).bind(format!("all-{index}-{n}")).bind(cell).execute(&pool).await.unwrap();
            let path = tenant_path(&root, tenant);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            sqlx::SqliteConnection::connect_with(
                &sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true),
            )
            .await
            .unwrap()
            .close()
            .await
            .unwrap();
            leases.acquire_assigned(tenant).await.unwrap();
            tenants.push((tenant, cell));
        }
        let manager = Arc::new(
            TenantDbManager::new(
                TenantDbConfig {
                    root: root.clone(),
                    ..Default::default()
                },
                leases,
            )
            .unwrap(),
        );
        orchestrators.push(
            MigrationOrchestrator::new(
                cell,
                manager.clone(),
                Arc::new(State { pool: pool.clone() }),
                vec![],
                Default::default(),
            )
            .unwrap(),
        );
        managers.push(manager);
    }
    let cold = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO tenants(id,slug,name,timezone,state) VALUES($1,'cold','Cold','UTC','COLD')",
    )
    .bind(cold)
    .execute(&pool)
    .await
    .unwrap();
    let (first, repeated) = tokio::join!(rollout::create_all(&pool), rollout::create_all(&pool));
    let id = first.unwrap().unwrap();
    assert_eq!(repeated.unwrap(), Some(id));
    assert_eq!(rollout::status(&pool, id).await.unwrap()["cohort"], "100%");
    assert_eq!(rollout::status(&pool, id).await.unwrap()["tenant_count"], 4);
    // An explicit all-tenants run also expands an existing canary rollout.
    sqlx::query("UPDATE schema_rollouts SET phase=0,canary_count=1,soak_seconds=600 WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(rollout::create_all(&pool).await.unwrap(), Some(id));
    assert_eq!(rollout::status(&pool, id).await.unwrap()["cohort"], "100%");
    let state = State { pool: pool.clone() };
    for (tenant, cell) in &tenants {
        let pending = state.pending(*cell, target).await.unwrap();
        assert_eq!(pending.len(), 2);
        assert!(pending.iter().any(|p| p.tenant_id == *tenant));
        assert!(pending
            .iter()
            .all(|p| tenants.contains(&(p.tenant_id, *cell))));
    }
    // Expired leases are still fenced even with the entire cohort admitted.
    sqlx::query("UPDATE tenant_leases SET renewed_at=clock_timestamp()-INTERVAL '2 minutes',expires_at=clock_timestamp()-INTERVAL '1 minute' WHERE tenant_id=$1")
        .bind(tenants[0].0).execute(&pool).await.unwrap();
    assert_eq!(state.pending(tenants[0].1, target).await.unwrap().len(), 1);
    sqlx::query("UPDATE tenant_leases SET expires_at=clock_timestamp()+INTERVAL '5 minutes' WHERE tenant_id=$1")
        .bind(tenants[0].0).execute(&pool).await.unwrap();
    for orchestrator in &orchestrators {
        let outcomes = orchestrator.run_pending().await.unwrap();
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(|o| o.succeeded()), "{outcomes:?}");
    }
    rollout::advance(&pool, target).await.unwrap();
    assert_eq!(
        rollout::status(&pool, id).await.unwrap()["state"],
        "COMPLETE"
    );
    assert_eq!(rollout::create_all(&pool).await.unwrap(), None);
    for (tenant, cell) in &tenants {
        let manager = &managers[usize::from(*cell != tenants[0].1)];
        let db = manager.open(*tenant).await.unwrap();
        let actual: i64 =
            sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success")
                .fetch_one(&db.read_pool().unwrap())
                .await
                .unwrap();
        assert_eq!(actual, target);
        db.close().await.unwrap();
    }
    // A tenant that wakes after completion is appended to the same rollout.
    sqlx::query(
        "UPDATE tenants SET state='ACTIVE',owner_cell=$2,ownership_generation=1 WHERE id=$1",
    )
    .bind(cold)
    .bind(tenants[0].1)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(rollout::create_all(&pool).await.unwrap(), Some(id));
    assert_eq!(rollout::status(&pool, id).await.unwrap()["tenant_count"], 5);
    assert_eq!(rollout::create_all(&pool).await.unwrap(), Some(id));
    assert_eq!(rollout::status(&pool, id).await.unwrap()["tenant_count"], 5);
    sqlx::query("UPDATE schema_rollouts SET state='HALTED',last_error='injected' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(rollout::create_all(&pool)
        .await
        .unwrap_err()
        .to_string()
        .contains("halted"));
    assert_eq!(rollout::status(&pool, id).await.unwrap()["state"], "HALTED");
    std::fs::remove_dir_all(root).unwrap();
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}
#[async_trait::async_trait]
impl MigrationHook for FailOnce {
    async fn before(
        &self,
        _context: MigrationContext,
        _connection: &mut sqlx::SqliteConnection,
    ) -> Result<(), gaming_cafe_api::error::AppError> {
        if !self.0.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return Err(gaming_cafe_api::error::AppError::Internal(
                "injected real runner hook failure".into(),
            ));
        }
        Ok(())
    }
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn stages_admit_only_the_cohort_halt_resume_and_recover_locks() {
    use sqlx::ConnectOptions;
    let url = std::env::var("CONTROL_TEST_DATABASE_URL").unwrap();
    let admin = PgPool::connect(&url).await.unwrap();
    let schema = format!("rollout_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(12)
        .connect_with(
            url.parse::<sqlx::postgres::PgConnectOptions>()
                .unwrap()
                .options([("search_path", schema.as_str())])
                .disable_statement_logging(),
        )
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    let cell = Uuid::new_v4();
    sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,'Rollout','http://rollout.invalid')")
        .bind(cell)
        .execute(&pool)
        .await
        .unwrap();
    let mut tenants = Vec::new();
    for n in 0..100 {
        let id = Uuid::new_v4();
        tenants.push(id);
        sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,ownership_generation,state) VALUES($1,$2,'Rollout','UTC',$3,1,'ACTIVE')").bind(id).bind(format!("rollout-{n}")).bind(cell).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO tenant_leases(tenant_id,owner_cell,ownership_generation,expires_at) VALUES($1,$2,1,clock_timestamp()+INTERVAL '5 minutes')").bind(id).bind(cell).execute(&pool).await.unwrap();
    }
    let target = target_schema_version();
    let id = rollout::create(&pool, target, &[tenants[0]], 0)
        .await
        .unwrap();
    let state = State { pool: pool.clone() };
    let pending = state.pending(cell, target).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].tenant_id, tenants[0]);
    let admission = state.admit(cell, pending[0], target).await.unwrap();
    assert!(state.admit(cell, pending[0], target).await.is_err());
    state
        .record_failure(cell, pending[0], target, "injected hook failure")
        .await
        .unwrap();
    assert_eq!(rollout::status(&pool, id).await.unwrap()["state"], "HALTED");
    assert!(state.pending(cell, target).await.unwrap().is_empty());
    rollout::advance(&pool, target).await.unwrap();
    assert_eq!(rollout::status(&pool, id).await.unwrap()["phase"], 0);
    drop(admission);
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    rollout::resume(&pool, id).await.unwrap();
    let root = std::env::temp_dir().join(format!("arena360-rollout-{}", Uuid::new_v4()));
    let path = tenant_path(&root, tenants[0]);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    use sqlx::Connection;
    let connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    connection.close().await.unwrap();
    let leases = Arc::new(LeaseClient::new(pool.clone(), cell, Default::default()).unwrap());
    leases.acquire_assigned(tenants[0]).await.unwrap();
    let jobs =
        gaming_cafe_api::background::BackgroundJobs::new(gaming_cafe_api::background::Limits {
            outbox_slots: 1,
            background_slots: 1,
            backfill_slots: 1,
        })
        .unwrap();
    let databases = Arc::new(
        TenantDbManager::new(
            TenantDbConfig {
                root: root.clone(),
                ..Default::default()
            },
            leases,
        )
        .unwrap()
        .with_background_jobs(jobs),
    );
    let orchestrator = MigrationOrchestrator::new(
        cell,
        databases.clone(),
        Arc::new(state.clone()),
        vec![Arc::new(FailOnce(std::sync::atomic::AtomicBool::new(
            false,
        )))],
        Default::default(),
    )
    .unwrap();
    let failed = orchestrator.run_pending().await.unwrap();
    assert_eq!(failed.len(), 1);
    assert!(!failed[0].succeeded());
    assert_eq!(rollout::status(&pool, id).await.unwrap()["state"], "HALTED");
    assert!(orchestrator.run_pending().await.unwrap().is_empty());
    rollout::resume(&pool, id).await.unwrap();
    let succeeded = orchestrator.run_pending().await.unwrap();
    assert_eq!(succeeded.len(), 1);
    assert!(succeeded[0].succeeded(), "{succeeded:?}");
    let db = databases.open(tenants[0]).await.unwrap();
    let actual: i64 = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success")
        .fetch_one(&db.read_pool().unwrap())
        .await
        .unwrap();
    assert_eq!(actual, target);
    for (phase, count) in [(0, 1), (1, 1), (2, 10), (3, 25), (4, 100)] {
        let prior: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM schema_rollout_tenants WHERE rollout_id=$1 AND state='SUCCEEDED'",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(rollout::status(&pool, id).await.unwrap()["phase"], phase);
        let pending = state.pending(cell, target).await.unwrap();
        assert_eq!(pending.len() as i64, count - prior);
        for tenant in pending {
            let permit = state.admit(cell, tenant, target).await.unwrap();
            state.record_version(cell, tenant, target).await.unwrap();
            drop(permit);
        }
        if phase == 0 {
            sqlx::query("UPDATE schema_rollouts SET soak_seconds=60 WHERE id=$1")
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
            rollout::advance(&pool, target).await.unwrap();
            assert_eq!(rollout::status(&pool, id).await.unwrap()["phase"], 0);
            sqlx::query("UPDATE schema_rollouts SET soak_seconds=0,stage_ready_at=clock_timestamp()-INTERVAL '61 seconds' WHERE id=$1").bind(id).execute(&pool).await.unwrap();
        }
        rollout::advance(&pool, target).await.unwrap();
    }
    assert_eq!(
        rollout::status(&pool, id).await.unwrap()["state"],
        "COMPLETE"
    );
    assert!(state.pending(cell, target).await.unwrap().is_empty());
    assert!(rollout::resume(&pool, id).await.is_err());
    // Completed control state survives a new State object and process-local leases.
    let fresh = State { pool: pool.clone() };
    assert!(fresh.pending(cell, target).await.unwrap().is_empty());
    db.close().await.unwrap();
    std::fs::remove_dir_all(root).unwrap();
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

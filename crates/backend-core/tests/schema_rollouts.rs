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

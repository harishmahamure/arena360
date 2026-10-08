#![cfg(feature = "duckdb-analytics")]
mod support;
use gaming_cafe_api::{
    historical::{archive, backfill, raw},
    metrics::Metrics,
    replication::{crypto::TenantKeys, ledger::PostgresLedger, wal},
};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires an isolated control database and native DuckDB"]
async fn archive_backfill_validates_revisions_restores_in_bounded_batches_and_recovers_checkpoint_failure(
) {
    let jobs =
        gaming_cafe_api::background::BackgroundJobs::new(gaming_cafe_api::background::Limits {
            outbox_slots: 1,
            background_slots: 1,
            backfill_slots: 1,
        })
        .unwrap();
    let fixture = support::SessionFixture::new_with_background_jobs(jobs.clone()).await;
    let db = fixture.tenant.db.clone();
    let tenant = db.tenant_id();
    let cell = Uuid::new_v4();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&std::env::var("CONTROL_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)")
        .bind(cell)
        .bind(cell.to_string())
        .bind(format!("http://{cell}.invalid"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,ownership_generation,state,schema_version) VALUES($1,$2,'Backfill','UTC',$3,1,'ACTIVE',$4)").bind(tenant).bind(tenant.to_string()).bind(cell).bind(gaming_cafe_api::tenancy::target_schema_version()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tenant_leases(tenant_id,owner_cell,ownership_generation,expires_at) VALUES($1,$2,1,clock_timestamp()+INTERVAL '30 minutes')").bind(tenant).bind(cell).execute(&pool).await.unwrap();
    let first = fixture
        .tenant
        .plan_transaction(fixture.player, fixture.plan, fixture.venue)
        .await;
    let second = fixture
        .tenant
        .plan_transaction(fixture.player, fixture.plan, fixture.venue)
        .await;
    db.with_immediate_writer(move|c|Box::pin(async move{for(id,amount)in[(first,123456i64),(second,789012i64)]{sqlx::query("UPDATE transactions SET transaction_date='2024-01-15T00:00:00.000000Z',created_at='2024-01-15T00:00:00.000000Z',updated_at='2024-01-15T00:00:00.000000Z',amount=?,paid_amount=?,cash_amount=? WHERE id=?").bind(amount).bind(amount).bind(amount).bind(id.to_string()).execute(&mut *c).await?;}Ok(())})).await.unwrap();
    let columns = raw::columns(&db.read_pool().unwrap(), "transactions")
        .await
        .unwrap();
    let payload: String =
        sqlx::query(&raw::row_select("transactions", &columns, "r.id=?").unwrap())
            .bind(first.to_string())
            .fetch_one(&db.read_pool().unwrap())
            .await
            .unwrap()
            .get("payload");
    let root = std::env::temp_dir().join(format!("arena-backfill-{tenant}"));
    let keys = TenantKeys::new(root.join("keys"));
    wal::durable_create(&keys.path(tenant), &[73; 32]).unwrap();
    let store = Arc::new(object_store::memory::InMemory::new());
    let ledger = Arc::new(PostgresLedger {
        pool: pool.clone(),
        cell_id: cell,
    });
    let metrics = Arc::new(Metrics::default());
    let archive_worker = archive::Worker {
        ledger: ledger.clone(),
        store: store.clone(),
        keys: keys.clone(),
        metrics: metrics.clone(),
    };
    let worker = backfill::Worker {
        ledger: ledger.clone(),
        store: store.clone(),
        keys: keys.clone(),
        metrics: metrics.clone(),
    };
    let original = archive::enqueue(&pool, tenant, "2024-01-01".parse().unwrap(), 60000, 1)
        .await
        .unwrap();
    assert!(backfill::enqueue(&pool, original.id, 60000, 1)
        .await
        .is_err());
    for _ in 0..50 {
        if archive_worker
            .advance(db.clone(), original.id)
            .await
            .unwrap()
        {
            break;
        }
    }
    assert_eq!(
        archive::get(&pool, original.id).await.unwrap().state,
        "COMPLETE"
    );
    // A correction creates a newer revision, while the second already-purged row exists only in the first revision.
    let mut value: serde_json::Value = serde_json::from_str(&payload).unwrap();
    for name in ["amount", "paid_amount", "cash_amount"] {
        value[name] = serde_json::json!(888888i64);
    }
    let corrected = serde_json::to_string(&value).unwrap();
    let source_columns = columns.clone();
    db.with_immediate_writer(move |c| {
        Box::pin(async move {
            let mut q = sqlx::QueryBuilder::<sqlx::Sqlite>::new("INSERT INTO transactions (");
            q.push(
                source_columns
                    .iter()
                    .map(|v| raw::identifier(&v.name).unwrap())
                    .collect::<Vec<_>>()
                    .join(","),
            );
            q.push(") SELECT ");
            let mut sep = q.separated(",");
            for col in &source_columns {
                sep.push("json_extract(")
                    .push_bind_unseparated(&corrected)
                    .push_unseparated(format!(",'$.{}')", col.name));
            }
            q.build().execute(c).await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let revision = archive::replan(&pool, original.id).await.unwrap();
    for _ in 0..50 {
        if archive_worker
            .advance(db.clone(), revision.id)
            .await
            .unwrap()
        {
            break;
        }
    }
    assert_eq!(
        archive::get(&pool, revision.id).await.unwrap().state,
        "COMPLETE"
    );
    // An additive nullable field is populated deterministically during private staging.
    db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("ALTER TABLE transactions ADD COLUMN backfill_note TEXT")
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let job = backfill::enqueue(&pool, revision.id, 60000, 1)
        .await
        .unwrap();
    assert!(archive::replan(&pool, revision.id).await.is_err());
    assert!(backfill::enqueue(&pool, revision.id, 60000, 1)
        .await
        .is_err());
    let before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM transactions WHERE transaction_date LIKE '2024-01-%'",
    )
    .fetch_one(&db.read_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(before, 0);
    let key = std::fs::read(keys.path(tenant)).unwrap();
    std::fs::remove_file(keys.path(tenant)).unwrap();
    assert!(worker.advance(db.clone(), job.id).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM transactions WHERE transaction_date LIKE '2024-01-%'"
        )
        .fetch_one(&db.read_pool().unwrap())
        .await
        .unwrap(),
        0
    );
    wal::durable_create(&keys.path(tenant), &key).unwrap();
    jobs.set_disk_zone(2);
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(100),
            worker.advance(db.clone(), job.id)
        )
        .await
        .is_err(),
        "disk pressure must queue P10 work"
    );
    jobs.set_disk_zone(0);
    assert!(!worker.advance(db.clone(), job.id).await.unwrap());
    let staged = backfill::get(&pool, job.id).await.unwrap();
    assert_eq!(staged.state, "BACKFILLING");
    assert_eq!(staged.expected_rows, Some(2));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM transactions WHERE transaction_date LIKE '2024-01-%'"
        )
        .fetch_one(&db.read_pool().unwrap())
        .await
        .unwrap(),
        0,
        "validation must not write live rows"
    );
    sqlx::query("UPDATE tenant_leases SET expires_at=clock_timestamp()-INTERVAL '1 second' WHERE tenant_id=$1").bind(tenant).execute(&pool).await.unwrap();
    assert!(worker.advance(db.clone(), job.id).await.is_err());
    sqlx::query("UPDATE tenant_leases SET expires_at=clock_timestamp()+INTERVAL '30 minutes' WHERE tenant_id=$1").bind(tenant).execute(&pool).await.unwrap();
    db.with_immediate_writer(|_| {
        Box::pin(async {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            Ok(())
        })
    })
    .await
    .unwrap();
    sqlx::query("UPDATE historical_backfills SET oltp_p99_target_milliseconds=1 WHERE id=$1")
        .bind(job.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(!worker.advance(db.clone(), job.id).await.unwrap());
    assert!(backfill::get(&pool, job.id)
        .await
        .unwrap()
        .last_error
        .unwrap()
        .contains("p99"));
    sqlx::query("UPDATE historical_backfills SET oltp_p99_target_milliseconds=60000,retry_after=clock_timestamp() WHERE id=$1").bind(job.id).execute(&pool).await.unwrap();
    // Lose local scratch; validated remote staging remains authoritative.
    let stage_root = db
        .path()
        .parent()
        .unwrap()
        .join("historical/backfill")
        .join(job.id.to_string());
    std::fs::remove_dir_all(&stage_root).unwrap();
    // Failure after the SQLite commit must resume from its checkpoint without duplicate outbox events.
    let fn_name = format!("reject_backfill_{}", job.id.simple());
    sqlx::query(&format!("CREATE FUNCTION {fn_name}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.id='{}'::uuid AND NEW.rows_processed>OLD.rows_processed THEN RAISE EXCEPTION 'Injected checkpoint mirror failure'; END IF; RETURN NEW; END $$",job.id)).execute(&pool).await.unwrap();
    sqlx::query(&format!("CREATE TRIGGER {fn_name} BEFORE UPDATE ON historical_backfills FOR EACH ROW EXECUTE FUNCTION {fn_name}()")).execute(&pool).await.unwrap();
    assert!(worker.advance(db.clone(), job.id).await.is_err());
    let local: i64 = sqlx::query_scalar(
        "SELECT rows_processed FROM archive_backfill_checkpoints WHERE backfill_id=?",
    )
    .bind(job.id.to_string())
    .fetch_one(&db.read_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(local, 1);
    assert_eq!(
        backfill::get(&pool, job.id).await.unwrap().rows_processed,
        0
    );
    let outbox_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM outbox_events WHERE event_type='historical.backfilled'",
    )
    .fetch_one(&db.read_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(outbox_after, 1);
    sqlx::query(&format!("DROP TRIGGER {fn_name} ON historical_backfills"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(&format!("DROP FUNCTION {fn_name}()"))
        .execute(&pool)
        .await
        .unwrap();
    for _ in 0..50 {
        let old = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM transactions WHERE transaction_date LIKE '2024-01-%'",
        )
        .fetch_one(&db.read_pool().unwrap())
        .await
        .unwrap();
        let done = worker.advance(db.clone(), job.id).await.unwrap();
        let new = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM transactions WHERE transaction_date LIKE '2024-01-%'",
        )
        .fetch_one(&db.read_pool().unwrap())
        .await
        .unwrap();
        assert!(new - old <= 1, "one live insert per configured batch");
        if done {
            break;
        }
    }
    let complete = backfill::get(&pool, job.id).await.unwrap();
    assert_eq!(complete.state, "COMPLETE");
    assert_eq!(complete.rows_processed, 2);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT SUM(amount) FROM transactions WHERE transaction_date LIKE '2024-01-%'"
        )
        .fetch_one(&db.read_pool().unwrap())
        .await
        .unwrap(),
        1677900
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM outbox_events WHERE event_type='historical.backfilled'"
        )
        .fetch_one(&db.read_pool().unwrap())
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT completed FROM archive_backfill_checkpoints WHERE backfill_id=?"
        )
        .bind(job.id.to_string())
        .fetch_one(&db.read_pool().unwrap())
        .await
        .unwrap(),
        1
    );
    assert!(!stage_root.exists());
    assert!(worker.advance(db.clone(), job.id).await.unwrap());
    // Repeated jobs accept identical rows; corrected live rows block private validation and are never overwritten.
    let retry = backfill::enqueue(&pool, revision.id, 60000, 1)
        .await
        .unwrap();
    db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE transactions SET amount=999999,paid_amount=999999,cash_amount=999999 WHERE id=?")
                .bind(first.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(worker.advance(db.clone(), retry.id).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT amount FROM transactions WHERE id=?")
            .bind(first.to_string())
            .fetch_one(&db.read_pool().unwrap())
            .await
            .unwrap(),
        999999
    );
    assert_eq!(
        backfill::get(&pool, retry.id).await.unwrap().rows_processed,
        0
    );
    sqlx::query("DELETE FROM tenants WHERE id=$1")
        .bind(tenant)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM cells WHERE id=$1")
        .bind(cell)
        .execute(&pool)
        .await
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

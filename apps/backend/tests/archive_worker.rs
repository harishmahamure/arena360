#![cfg(feature = "duckdb-analytics")]
mod support;
use gaming_cafe_api::{
    historical::{
        archive::{self, Worker},
        handoff,
        objects::{self, Object},
        raw,
    },
    metrics::Metrics,
    replication::{crypto::TenantKeys, ledger::PostgresLedger, wal},
};
use object_store::{ObjectStore, ObjectStoreExt};
use sqlx::Row;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use uuid::Uuid;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires an isolated control-plane database and native DuckDB"]
async fn verified_archive_purges_in_batches_resumes_control_failure_and_preserves_corrections() {
    let jobs =
        gaming_cafe_api::background::BackgroundJobs::new(gaming_cafe_api::background::Limits {
            outbox_slots: 1,
            background_slots: 1,
            backfill_slots: 1,
        })
        .unwrap();
    let fixture = support::SessionFixture::new_with_background_jobs(jobs).await;
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
    sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,ownership_generation,state,schema_version) VALUES($1,$2,'Archive','UTC',$3,1,'ACTIVE',$4)").bind(tenant).bind(tenant.to_string()).bind(cell).bind(gaming_cafe_api::tenancy::target_schema_version()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tenant_leases(tenant_id,owner_cell,ownership_generation,expires_at) VALUES($1,$2,1,clock_timestamp()+INTERVAL '30 minutes')").bind(tenant).bind(cell).execute(&pool).await.unwrap();
    let first = fixture
        .tenant
        .plan_transaction(fixture.player, fixture.plan, fixture.venue)
        .await;
    let second = fixture
        .tenant
        .plan_transaction(fixture.player, fixture.plan, fixture.venue)
        .await;
    db.with_immediate_writer(move|c|Box::pin(async move{for (id,amount) in [(first,123456i64),(second,789012i64)]{sqlx::query("UPDATE transactions SET transaction_date='2024-01-15T00:00:00.000000Z',created_at='2024-01-15T00:00:00.000000Z',updated_at='2024-01-15T00:00:00.000000Z',amount=?,paid_amount=?,cash_amount=? WHERE id=?").bind(amount).bind(amount).bind(amount).bind(id.to_string()).execute(&mut *c).await?;}Ok(())})).await.unwrap();
    let overlap_session = Uuid::new_v4();
    let overlap_shift = Uuid::new_v4();
    let player = fixture.player;
    let balance = fixture.balance;
    let device = fixture.device;
    let venue = fixture.venue;
    db.with_immediate_writer(move |c| Box::pin(async move {
        sqlx::query("INSERT INTO shifts(id,user_id,location_id,clock_in,clock_out,status,created_at,updated_at) VALUES(?,?,?,'2024-01-15T00:00:00.000000Z','2026-09-15T00:00:00.000000Z','closed','2024-01-15T00:00:00.000000Z','2026-09-15T00:00:00.000000Z')")
            .bind(overlap_shift.to_string()).bind(player.to_string()).bind(venue.to_string()).execute(&mut *c).await?;
        sqlx::query("INSERT INTO usage_sessions(id,player_id,balance_id,device_id,location_id,shift_id,start_time,end_time,wallet_minutes_at_start,end_reason,created_at,updated_at) VALUES(?,?,?,?,?,?,'2024-01-15T00:00:00.000000Z','2026-09-15T00:00:00.000000Z',120,'voluntary','2024-01-15T00:00:00.000000Z','2026-09-15T00:00:00.000000Z')")
            .bind(overlap_session.to_string()).bind(player.to_string()).bind(balance.to_string()).bind(device.to_string()).bind(venue.to_string()).bind(overlap_shift.to_string()).execute(&mut *c).await?;
        Ok(())
    })).await.unwrap();
    let root = std::env::temp_dir().join(format!("arena-archive-{tenant}"));
    let keys = TenantKeys::new(root.join("keys"));
    wal::durable_create(&keys.path(tenant), &[61; 32]).unwrap();
    let store = Arc::new(object_store::memory::InMemory::new());
    let worker = Arc::new(Worker {
        ledger: Arc::new(PostgresLedger {
            pool: pool.clone(),
            cell_id: cell,
        }),
        store: store.clone(),
        keys: keys.clone(),
        metrics: Arc::new(Metrics::default()),
    });
    assert!(
        archive::enqueue(&pool, tenant, "2026-09-01".parse().unwrap(), 100, 1)
            .await
            .is_err()
    );
    let job = archive::enqueue(&pool, tenant, "2024-01-01".parse().unwrap(), 100, 2)
        .await
        .unwrap();
    let hot_key =
        object_store::path::Path::from(format!("tenants/{tenant}/hot/2024/01/current.parquet"));
    let orphan_key =
        object_store::path::Path::from(format!("tenants/{tenant}/hot/2024/01/orphan.parquet"));
    let adjacent_key =
        object_store::path::Path::from(format!("tenants/{tenant}/hot/2024/02/other.parquet"));
    for key in [&hot_key, &orphan_key, &adjacent_key] {
        store
            .put(key, b"rebuildable".to_vec().into())
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO hot_month_manifests(tenant_id,period_start,period_end,timezone,ownership_generation,source_watermark,projection_version,objects) VALUES($1,'2024-01-01','2024-02-01','UTC',1,0,1,'[]')")
        .bind(tenant).execute(&pool).await.unwrap();
    assert!(worker.handoff(db.clone(), job.id).await.is_err());
    assert!(store.head(&hot_key).await.is_ok());
    assert!(worker.purge(db.clone(), job.id).await.is_err());
    worker.export(db.clone(), job.id).await.unwrap();
    let uploaded = archive::get(&pool, job.id).await.unwrap();
    assert_eq!(uploaded.state, "UPLOADED");
    assert_eq!(uploaded.row_count, Some(2));
    assert!(worker.purge(db.clone(), job.id).await.is_err());
    worker.verify(db.clone(), job.id).await.unwrap();
    for table in ["usage_sessions", "shifts"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&db.read_pool().unwrap())
            .await
            .unwrap();
        assert_eq!(count, 1, "overlapping {table} must stay live");
    }
    let evidence: Vec<Object> = serde_json::from_value(uploaded.objects).unwrap();
    let object = evidence.iter().find(|o| o.table == "transactions").unwrap();
    // An unavailable verified archive must preserve the existing hot copy.
    let archive_key = object_store::path::Path::from(object.key.as_str());
    let encrypted = store
        .get(&archive_key)
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    store
        .put(&archive_key, b"corrupt".to_vec().into())
        .await
        .unwrap();
    assert!(worker.handoff(db.clone(), job.id).await.is_err());
    assert!(store.head(&hot_key).await.is_ok());
    let state: String = sqlx::query_scalar(
        "SELECT state FROM hot_month_manifests WHERE tenant_id=$1 AND period_start='2024-01-01'",
    )
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, "READY");
    store.put(&archive_key, encrypted.into()).await.unwrap();
    let mut reader = pool.begin().await.unwrap();
    handoff::lock_month(&mut reader, tenant, job.period_start, true)
        .await
        .unwrap();
    let task_worker = worker.clone();
    let task_db = db.clone();
    let archive_id = job.id;
    let mut retirement =
        tokio::spawn(async move { task_worker.handoff(task_db, archive_id).await });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut retirement)
            .await
            .is_err()
    );
    assert!(store.head(&hot_key).await.is_ok());
    reader.rollback().await.unwrap();
    retirement.await.unwrap().unwrap();
    worker.handoff(db.clone(), job.id).await.unwrap();
    assert!(store.head(&hot_key).await.is_err());
    assert!(store.head(&orphan_key).await.is_err());
    assert!(store.head(&adjacent_key).await.is_ok());
    assert!(archive::get(&pool, job.id)
        .await
        .unwrap()
        .hot_cleaned_at
        .is_some());
    let state: String = sqlx::query_scalar(
        "SELECT state FROM hot_month_manifests WHERE tenant_id=$1 AND period_start='2024-01-01'",
    )
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, "RETIRED");
    // A crash after retirement/deletion but before the progress update leaves a retryable prefix.
    sqlx::query("UPDATE archive_manifests SET hot_cleaned_at=NULL WHERE id=$1")
        .bind(job.id)
        .execute(&pool)
        .await
        .unwrap();
    store
        .put(&orphan_key, b"remaining upload".to_vec().into())
        .await
        .unwrap();
    worker.handoff(db.clone(), job.id).await.unwrap();
    assert!(store.head(&orphan_key).await.is_err());
    assert!(store.head(&adjacent_key).await.is_ok());
    let file = root.join("transactions.parquet");
    objects::download(store.as_ref(), &keys.read(tenant).unwrap(), object, &file)
        .await
        .unwrap();
    let archived = raw::batch(file, None, 100).await.unwrap();
    assert_eq!(archived.len(), 2);
    assert_eq!(
        archived
            .iter()
            .map(
                |(_, payload, _)| serde_json::from_str::<serde_json::Value>(payload).unwrap()
                    ["amount"]
                    .as_i64()
                    .unwrap()
            )
            .sum::<i64>(),
        912468
    );
    std::fs::remove_file(keys.path(tenant)).unwrap();
    assert!(worker.purge(db.clone(), job.id).await.is_err());
    wal::durable_create(&keys.path(tenant), &[61; 32]).unwrap();
    // A corrected live row must not be removed using an older verified copy.
    db.with_immediate_writer(move|c|Box::pin(async move{sqlx::query("UPDATE transactions SET amount=777000,paid_amount=777000,cash_amount=777000 WHERE id=?").bind(first.to_string()).execute(c).await?;Ok(())})).await.unwrap();
    assert!(worker.purge(db.clone(), job.id).await.is_err());
    let amount: i64 = sqlx::query_scalar("SELECT amount FROM transactions WHERE id=?")
        .bind(first.to_string())
        .fetch_one(&db.read_pool().unwrap())
        .await
        .unwrap();
    assert_eq!(amount, 777000);
    db.with_immediate_writer(move|c|Box::pin(async move{sqlx::query("UPDATE transactions SET amount=123456,paid_amount=123456,cash_amount=123456 WHERE id=?").bind(first.to_string()).execute(c).await?;Ok(())})).await.unwrap();
    // Incoming references outside this month block parent deletion, including cascades.
    let ledger = Uuid::new_v4();
    let balance = fixture.balance;
    let player = fixture.player;
    db.with_immediate_writer(move|c|Box::pin(async move{let at=gaming_cafe_api::time::format_sqlite_timestamp(&chrono::Utc::now()).unwrap();sqlx::query("INSERT INTO player_plan_ledger(id,balance_id,player_id,delta_minutes,reason,transaction_id,balance_after,expiry_after,created_at) VALUES(?,?,?,1,'recharge',?,121,'2099-01-01T00:00:00.000000Z',?)").bind(ledger.to_string()).bind(balance.to_string()).bind(player.to_string()).bind(first.to_string()).bind(at).execute(c).await?;Ok(())})).await.unwrap();
    assert!(worker.purge(db.clone(), job.id).await.is_err());
    db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("DELETE FROM player_plan_ledger WHERE id=?")
                .bind(ledger.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    // Actual foreground latency pauses the purge and reduces its batch size.
    db.with_immediate_writer(|_| {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            Ok(())
        })
    })
    .await
    .unwrap();
    sqlx::query("UPDATE archive_manifests SET oltp_p99_target_milliseconds=1 WHERE id=$1")
        .bind(job.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(!worker.purge(db.clone(), job.id).await.unwrap());
    assert_eq!(archive::get(&pool, job.id).await.unwrap().rows_purged, 0);
    assert_eq!(archive::get(&pool, job.id).await.unwrap().batch_rows, 1);
    sqlx::query("UPDATE archive_manifests SET oltp_p99_target_milliseconds=100 WHERE id=$1")
        .bind(job.id)
        .execute(&pool)
        .await
        .unwrap();
    // Fail the CP progress write after SQLite committed the delete/checkpoint.
    let function = format!("archive_fault_{}", job.id.simple());
    let trigger = format!("archive_fault_trigger_{}", job.id.simple());
    sqlx::raw_sql(&format!("CREATE FUNCTION {function}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.id='{}'::uuid AND NEW.rows_purged>0 THEN RAISE EXCEPTION 'injected control progress failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER {trigger} BEFORE UPDATE ON archive_manifests FOR EACH ROW EXECUTE FUNCTION {function}();",job.id)).execute(&pool).await.unwrap();
    assert!(worker.purge(db.clone(), job.id).await.is_err());
    let local =
        sqlx::query("SELECT rows_deleted FROM archive_purge_checkpoints WHERE archive_id=?")
            .bind(job.id.to_string())
            .fetch_one(&db.read_pool().unwrap())
            .await
            .unwrap();
    assert_eq!(local.get::<i64, _>(0), 1);
    assert_eq!(archive::get(&pool, job.id).await.unwrap().rows_purged, 0);
    assert_eq!(archive::get(&pool, job.id).await.unwrap().batch_rows, 1);
    sqlx::raw_sql(&format!(
        "DROP TRIGGER {trigger} ON archive_manifests;DROP FUNCTION {function}();"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let flag = running.clone();
    let traffic_db = db.clone();
    let plan = fixture.plan;
    let traffic = tokio::spawn(async move {
        while flag.load(Ordering::Acquire) {
            traffic_db
                .with_immediate_writer(move |c| {
                    Box::pin(async move {
                        sqlx::query("UPDATE plans SET price=price WHERE id=?")
                            .bind(plan.to_string())
                            .execute(c)
                            .await?;
                        Ok(())
                    })
                })
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    });
    for attempt in 0..100 {
        if worker.purge(db.clone(), job.id).await.unwrap() {
            break;
        }
        assert!(attempt < 99, "purge did not finish");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    running.store(false, Ordering::Release);
    traffic.await.unwrap();
    let completed = archive::get(&pool, job.id).await.unwrap();
    assert_eq!(completed.state, "COMPLETE");
    assert_eq!(completed.rows_purged, 2);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM transactions WHERE transaction_date LIKE '2024-01%'",
    )
    .fetch_one(&db.read_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(count, 0);
    let p99 = db.foreground_write_p99_micros();
    println!("Local foreground write p99 during archive fixture: {p99} us (budget 100000 us)");
    assert!(p99 <= 100000);
    // Completed revisions remain available when the month receives corrected/backfilled data.
    let replanned = archive::replan(&pool, job.id).await.unwrap();
    assert!(replanned.revision > completed.revision);
    assert!(archive::get(&pool, job.id)
        .await
        .unwrap()
        .superseded_at
        .is_some());
    assert!(archive::get(&pool, job.id)
        .await
        .unwrap()
        .checksum_sha256
        .is_some());
    fixture.close().await;
    std::fs::remove_dir_all(root).unwrap();
}

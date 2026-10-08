#![cfg(feature = "duckdb-analytics")]
mod support;
use gaming_cafe_api::{
    historical::{archive, export_worker, exports, hot, objects, raw},
    metrics::Metrics,
    replication::{crypto::TenantKeys, ledger::PostgresLedger, wal},
};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires an isolated control database and native DuckDB"]
async fn isolated_exports_combine_hot_and_archive_revisions_with_exact_money_and_signed_cold_downloads(
) {
    let fixture = support::SessionFixture::new().await;
    let db = fixture.tenant.db.clone();
    let tenant = db.tenant_id();
    let cell = Uuid::new_v4();
    let user = Uuid::new_v4();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(12)
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
    sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,ownership_generation,state,schema_version) VALUES($1,$2,'Export','UTC',$3,1,'ACTIVE',$4)").bind(tenant).bind(tenant.to_string()).bind(cell).bind(gaming_cafe_api::tenancy::target_schema_version()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tenant_leases(tenant_id,owner_cell,ownership_generation,expires_at) VALUES($1,$2,1,clock_timestamp()+INTERVAL '30 minutes')").bind(tenant).bind(cell).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO users(id,username,password_hash) VALUES($1,$2,'fixture')")
        .bind(user)
        .bind(user.to_string())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO organization_memberships(tenant_id,user_id,role,permissions) VALUES($1,$2,'admin','[]')").bind(tenant).bind(user).execute(&pool).await.unwrap();
    let root = std::env::temp_dir().join(format!("arena-exports-{tenant}"));
    std::fs::create_dir_all(root.join("objects")).unwrap();
    let store = Arc::new(
        object_store::local::LocalFileSystem::new_with_prefix(root.join("objects")).unwrap(),
    );
    let keys = TenantKeys::new(root.join("keys"));
    wal::durable_create(&keys.path(tenant), &[72; 32]).unwrap();
    let metrics = Arc::new(Metrics::default());
    let ledger = Arc::new(PostgresLedger {
        pool: pool.clone(),
        cell_id: cell,
    });
    let archive_worker = archive::Worker {
        ledger: ledger.clone(),
        store: store.clone(),
        keys: keys.clone(),
        metrics: metrics.clone(),
    };
    let mut ids = vec![];
    for _ in 0..4 {
        ids.push(
            fixture
                .tenant
                .plan_transaction(fixture.player, fixture.plan, fixture.venue)
                .await,
        );
    }
    let original = ids.clone();
    db.with_immediate_writer(move|c|Box::pin(async move{for (index,id) in ids.iter().enumerate(){let amount=[123456i64,789012,54321,100100][index];let at=if index==3{"2024-02-15T00:00:00.000000Z"}else{"2024-01-15T00:00:00.000000Z"};sqlx::query("UPDATE transactions SET transaction_date=?,created_at=?,updated_at=?,amount=?,paid_amount=?,cash_amount=? WHERE id=?").bind(at).bind(at).bind(at).bind(amount).bind(amount).bind(amount).bind(id.to_string()).execute(&mut *c).await?;}Ok(())})).await.unwrap();
    let archive1 = archive::enqueue(&pool, tenant, "2024-01-01".parse().unwrap(), 100, 100)
        .await
        .unwrap();
    archive_worker
        .export(db.clone(), archive1.id)
        .await
        .unwrap();
    archive_worker
        .verify(db.clone(), archive1.id)
        .await
        .unwrap();
    let source = archive::get(&pool, archive1.id).await.unwrap();
    let evidence: Vec<objects::Object> = serde_json::from_value(source.objects).unwrap();
    let object = evidence.iter().find(|o| o.table == "transactions").unwrap();
    let raw_file = root.join("original.parquet");
    objects::download(
        store.as_ref(),
        &keys.read(tenant).unwrap(),
        object,
        &raw_file,
    )
    .await
    .unwrap();
    let original_rows = raw::batch(raw_file, None, 100).await.unwrap();
    archive_worker
        .handoff(db.clone(), archive1.id)
        .await
        .unwrap();
    for _ in 0..100 {
        if archive_worker.purge(db.clone(), archive1.id).await.unwrap() {
            break;
        }
    }
    assert_eq!(
        archive::get(&pool, archive1.id).await.unwrap().state,
        "COMPLETE"
    );
    // Restore a correction and a tombstone; a third purged row exists only in the older revision.
    let columns = object.columns.clone();
    let first = original[0].to_string();
    let second = original[1].to_string();
    db.with_immediate_writer(move |c| {
        Box::pin(async move {
            for (id, payload, _) in original_rows {
                if id != first && id != second {
                    continue;
                }
                let mut value: serde_json::Value = serde_json::from_str(&payload).unwrap();
                if id == first {
                    for field in ["amount", "paid_amount", "cash_amount"] {
                        value[field] = serde_json::json!(888888);
                    }
                } else {
                    value["deleted_at"] = serde_json::json!("2026-10-08T00:00:00.000000Z");
                }
                let sql = format!(
                    "INSERT INTO transactions({}) VALUES({})",
                    columns
                        .iter()
                        .map(|c| raw::identifier(&c.name).unwrap())
                        .collect::<Vec<_>>()
                        .join(","),
                    vec!["?"; columns.len()].join(",")
                );
                let mut insert = sqlx::query(&sql);
                for column in &columns {
                    if column.kind == "INTEGER" {
                        insert = insert.bind(value[&column.name].as_i64());
                    } else {
                        insert = insert.bind(value[&column.name].as_str().map(str::to_owned));
                    }
                }
                insert.execute(&mut *c).await?;
            }
            Ok(())
        })
    })
    .await
    .unwrap();
    let archive2 = archive::replan(&pool, archive1.id).await.unwrap();
    archive_worker
        .export(db.clone(), archive2.id)
        .await
        .unwrap();
    archive_worker
        .verify(db.clone(), archive2.id)
        .await
        .unwrap();
    archive_worker
        .handoff(db.clone(), archive2.id)
        .await
        .unwrap();
    for _ in 0..100 {
        if archive_worker.purge(db.clone(), archive2.id).await.unwrap() {
            break;
        }
    }
    assert_eq!(
        archive::get(&pool, archive2.id).await.unwrap().state,
        "COMPLETE"
    );
    let writer = hot::Writer {
        ledger,
        store: store.clone(),
        keys: keys.clone(),
        metrics: metrics.clone(),
        staging_root: root.join("hot"),
    };
    writer
        .refresh(db.clone(), "2025-08-01T00:00:00Z".parse().unwrap())
        .await
        .unwrap();
    sqlx::query("UPDATE tenants SET state='COLD',owner_cell=NULL WHERE id=$1")
        .bind(tenant)
        .execute(&pool)
        .await
        .unwrap();
    let request = |format: &str| exports::Request {
        start: "2024-01-01T00:00:00Z".parse().unwrap(),
        end: "2024-03-01T00:00:00Z".parse().unwrap(),
        format: format.into(),
        filters: exports::Filters {
            table: "transactions".into(),
            location_id: None,
            daily_revenue: false,
        },
    };
    assert!(serde_json::from_value::<exports::Request>(serde_json::json!({"start":"2024-01-01T00:00:00+05:30","end":"2024-03-01T00:00:00Z","format":"CSV","filters":{"table":"transactions"}})).is_err());
    let mut csv_job = None;
    for format in ["CSV", "CSV_GZ", "PARQUET"] {
        let job = exports::enqueue(&pool, tenant, user, request(format))
            .await
            .unwrap();
        let claimed = exports::claim_specific(&pool, cell, job.id)
            .await
            .unwrap()
            .unwrap();
        let token = claimed.worker_token.unwrap();
        let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_export_worker"));
        command
            .args([
                "--run-job",
                &job.id.to_string(),
                "--token",
                &token.to_string(),
            ])
            .env(
                "CONTROL_DATABASE_URL",
                std::env::var("CONTROL_TEST_DATABASE_URL").unwrap(),
            )
            .env("CELL_ID", cell.to_string())
            .env("EXPORT_STAGING_DIR", root.join("staging"))
            .env("EXPORT_TEST_OBJECT_DIR", root.join("objects"))
            .env("REPLICATION_KEY_DIR", root.join("keys"))
            .env("DISK_PRESSURE_MONITOR", "false")
            .env("RUST_ENV", "test")
            .env("NODE_ENV", "test")
            .env("ENVIRONMENT", "test")
            .kill_on_drop(true);
        if format == "CSV" {
            command.env("EXPORT_TEST_BLOCK_MS", "25000");
        } else {
            command.env_remove("EXPORT_TEST_BLOCK_MS");
        }
        let child = command.spawn().unwrap();
        assert_ne!(child.id().unwrap(), std::process::id());
        if format == "CSV" {
            tokio::time::sleep(Duration::from_secs(22)).await;
            let renewed = exports::get(&pool, tenant, job.id).await.unwrap();
            assert!(
                renewed.worker_expires_at.unwrap()
                    > claimed.worker_expires_at.unwrap() + chrono::Duration::seconds(10),
                "an independent heartbeat must renew while the worker main thread is blocked"
            );
        }
        let output = tokio::time::timeout(Duration::from_secs(90), child.wait_with_output())
            .await
            .unwrap()
            .unwrap();
        assert!(
            output.status.success(),
            "isolated export failed: {output:?}"
        );
        let ready = exports::get(&pool, tenant, job.id).await.unwrap();
        assert_eq!(ready.state, "READY");
        assert_eq!(ready.row_count, Some(3));
        assert!(exports::renew(&pool, job.id, token).await.is_err());
        let result: objects::Object =
            serde_json::from_value(ready.result_object.clone().unwrap()).unwrap();
        let file = root.join(format!("result-{format}"));
        objects::download(store.as_ref(), &keys.read(tenant).unwrap(), &result, &file)
            .await
            .unwrap();
        if format == "PARQUET" {
            let path = file.clone();
            let total: String = tokio::task::spawn_blocking(move || {
                let db = duckdb::Connection::open_in_memory().unwrap();
                db.query_row(
                    &format!(
                        "SELECT CAST(SUM(amount) AS VARCHAR) FROM read_parquet({})",
                        raw::literal(path.to_str().unwrap())
                    ),
                    [],
                    |r| r.get(0),
                )
                .unwrap()
            })
            .await
            .unwrap();
            assert_eq!(
                total.parse::<rust_decimal::Decimal>().unwrap(),
                "104.3309".parse::<rust_decimal::Decimal>().unwrap()
            );
        } else {
            let text = if format == "CSV_GZ" {
                use std::io::Read;
                let mut text = String::new();
                flate2::read::GzDecoder::new(std::fs::File::open(file).unwrap())
                    .read_to_string(&mut text)
                    .unwrap();
                text
            } else {
                std::fs::read_to_string(file).unwrap()
            };
            let mut rows = text.lines();
            let header = rows.next().unwrap();
            let amount = header.split(',').position(|c| c == "amount").unwrap();
            let total: rust_decimal::Decimal = rows
                .map(|r| {
                    r.split(',')
                        .nth(amount)
                        .unwrap()
                        .parse::<rust_decimal::Decimal>()
                        .unwrap()
                })
                .sum();
            assert_eq!(total, "104.3309".parse::<rust_decimal::Decimal>().unwrap());
        }
        if format == "CSV" {
            csv_job = Some(ready);
        }
    }
    // Stale tokens and all three admission limits are enforced in PostgreSQL.
    sqlx::query("UPDATE historical_export_limits SET platform_slots=2,cell_slots=1,tenant_slots=1")
        .execute(&pool)
        .await
        .unwrap();
    let a = exports::enqueue(&pool, tenant, user, request("CSV"))
        .await
        .unwrap();
    let b = exports::enqueue(&pool, tenant, user, request("CSV"))
        .await
        .unwrap();
    let owner = exports::claim_specific(&pool, cell, a.id)
        .await
        .unwrap()
        .unwrap();
    let scratch = root
        .join("staging")
        .join(format!("{}-{}", a.id, owner.worker_token.unwrap()));
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::write(
        scratch.join("export-owner.json"),
        serde_json::to_vec(&serde_json::json!({"id":a.id,"token":owner.worker_token.unwrap()}))
            .unwrap(),
    )
    .unwrap();
    std::fs::File::open(&scratch)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
        .unwrap();
    assert_eq!(
        exports::cleanup_staging(&pool, &root.join("staging"))
            .await
            .unwrap(),
        0
    );
    assert!(exports::claim_specific(&pool, cell, b.id)
        .await
        .unwrap()
        .is_none());
    let other_cell = Uuid::new_v4();
    sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)")
        .bind(other_cell)
        .bind(other_cell.to_string())
        .bind(format!("http://{other_cell}.invalid"))
        .execute(&pool)
        .await
        .unwrap();
    assert!(exports::claim_specific(&pool, other_cell, b.id)
        .await
        .unwrap()
        .is_none());
    let other = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO tenants(id,slug,name,timezone,state) VALUES($1,$2,'Other','UTC','COLD')",
    )
    .bind(other)
    .bind(other.to_string())
    .execute(&pool)
    .await
    .unwrap();
    let c = exports::enqueue(&pool, other, user, request("CSV"))
        .await
        .unwrap();
    assert!(exports::claim_specific(&pool, cell, c.id)
        .await
        .unwrap()
        .is_none());
    let second = exports::claim_specific(&pool, other_cell, c.id)
        .await
        .unwrap()
        .unwrap();
    let third_cell = Uuid::new_v4();
    sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)")
        .bind(third_cell)
        .bind(third_cell.to_string())
        .bind(format!("http://{third_cell}.invalid"))
        .execute(&pool)
        .await
        .unwrap();
    let d = exports::enqueue(&pool, other, user, request("CSV"))
        .await
        .unwrap();
    assert!(exports::claim_specific(&pool, third_cell, d.id)
        .await
        .unwrap()
        .is_none());
    sqlx::query("UPDATE historical_exports SET worker_expires_at=clock_timestamp()-INTERVAL '1 second' WHERE id=$1").bind(a.id).execute(&pool).await.unwrap();
    assert!(exports::renew(&pool, a.id, owner.worker_token.unwrap())
        .await
        .is_err());
    let replacement = exports::claim_specific(&pool, cell, a.id)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(replacement.worker_token, owner.worker_token);
    assert_eq!(
        exports::cleanup_staging(&pool, &root.join("staging"))
            .await
            .unwrap(),
        1
    );
    assert!(!scratch.exists());
    let engine = export_worker::Worker {
        pool: pool.clone(),
        cell,
        store: store.clone(),
        keys: keys.clone(),
        staging_root: root.join("staging"),
        metrics,
    };
    assert!(engine.run(a.id, owner.worker_token.unwrap()).await.is_err());
    exports::release(
        &pool,
        a.id,
        replacement.worker_token.unwrap(),
        "fixture cleanup",
    )
    .await
    .unwrap();
    exports::release(&pool, c.id, second.worker_token.unwrap(), "fixture cleanup")
        .await
        .unwrap();
    for job in [a, b, c, d] {
        exports::cancel(&pool, job.tenant_id, job.id).await.unwrap();
    }
    sqlx::query("UPDATE historical_export_limits SET platform_slots=4,cell_slots=2,tenant_slots=1")
        .execute(&pool)
        .await
        .unwrap();
    // The new HTTP surface works with legacy REST disabled and no TenantDb manager.
    let mut settings = support::settings();
    settings.legacy_rest_enabled = false;
    settings.tenant_data_dir = root.join("api");
    let secret = settings.jwt_secret.clone();
    let mut state = gaming_cafe_api::app::build_state_with_settings(Arc::new(settings)).await;
    let mutable = Arc::get_mut(&mut state).unwrap();
    mutable.control_db = Some(pool.clone());
    mutable.tenant_dbs = None;
    let app = gaming_cafe_api::app::build_router(state);
    let jwt=jsonwebtoken::encode(&jsonwebtoken::Header::default(),&serde_json::json!({"sub":user,"userId":user,"tenantId":tenant,"roles":["admin"],"permissions":[],"allowedTenants":[tenant],"orgIds":[tenant],"appId":"game-zone-backend","aud":"gamezone","iss":"gamezone","iat":chrono::Utc::now().timestamp(),"exp":(chrono::Utc::now()+chrono::Duration::minutes(15)).timestamp()}),&jsonwebtoken::EncodingKey::from_secret(secret.as_bytes())).unwrap();
    let ready = csv_job.unwrap();
    let (status, link) = support::request(
        app.clone(),
        "GET",
        &format!("/historical/exports/{}/download", ready.id),
        Some(&jwt),
        None,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, 200, "{link}");
    let url = link["data"]["url"].as_str().unwrap();
    let _environment = EnvGuard::set(vec![
        (
            "EXPORT_TEST_OBJECT_DIR",
            root.join("objects").into_os_string(),
        ),
        ("REPLICATION_KEY_DIR", root.join("keys").into_os_string()),
        ("DISK_PRESSURE_MONITOR", "false".into()),
        ("RUST_ENV", "test".into()),
        ("NODE_ENV", "test".into()),
        ("ENVIRONMENT", "test".into()),
    ]);
    let (status, csv) =
        support::request(app.clone(), "GET", url, None, None, serde_json::json!({})).await;
    assert_eq!(status, 200, "{csv}");
    assert_eq!(csv["raw"].as_str().unwrap().lines().count(), 4);
    assert!(csv["raw"].as_str().unwrap().contains("88.8888"));
    // Verify the public signature before the route touches object storage.
    let bad = url.replace("signature=", "signature=00");
    let (status, _) =
        support::request(app.clone(), "GET", &bad, None, None, serde_json::json!({})).await;
    assert_eq!(status, 403);
    let expiry = (chrono::Utc::now() + chrono::Duration::minutes(5)).timestamp();
    let checksum = ready.checksum_sha256.as_deref().unwrap();
    let signature = exports::sign(&secret, tenant, ready.id, expiry, checksum);
    assert!(
        exports::verify_signature(&secret, other, ready.id, expiry, checksum, &signature).is_err()
    );
    assert!(
        exports::verify_signature("wrong", tenant, ready.id, expiry, checksum, &signature).is_err()
    );
    let expired = chrono::Utc::now().timestamp() - 1;
    let signature = exports::sign(&secret, tenant, ready.id, expired, checksum);
    let (status, _) = support::request(
        app.clone(),
        "GET",
        &format!(
            "/historical/downloads/{tenant}/{}?expires={expired}&signature={signature}",
            ready.id
        ),
        None,
        None,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, 403);
    let (status, _) = support::request(
        app.clone(),
        "GET",
        &format!("/historical/exports/{}", ready.id),
        None,
        None,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, 401);
    sqlx::query(
        "UPDATE organization_memberships SET is_active=false WHERE tenant_id=$1 AND user_id=$2",
    )
    .bind(tenant)
    .bind(user)
    .execute(&pool)
    .await
    .unwrap();
    let (status, _) = support::request(
        app.clone(),
        "GET",
        &format!("/historical/exports/{}", ready.id),
        Some(&jwt),
        None,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, 401);
    let (status, _) =
        support::request(app.clone(), "GET", url, None, None, serde_json::json!({})).await;
    assert_eq!(
        status, 403,
        "revoked membership must invalidate its download grants"
    );
    fixture.tenant.close().await;
    std::fs::remove_dir_all(root).unwrap();
}

struct EnvGuard(Vec<(String, Option<std::ffi::OsString>)>);
impl EnvGuard {
    fn set(values: Vec<(&str, std::ffi::OsString)>) -> Self {
        let previous = values
            .iter()
            .map(|(name, _)| ((*name).to_string(), std::env::var_os(name)))
            .collect();
        for (name, value) in values {
            std::env::set_var(name, value);
        }
        Self(previous)
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (name, value) in &self.0 {
            if let Some(value) = value {
                std::env::set_var(name, value);
            } else {
                std::env::remove_var(name);
            }
        }
    }
}

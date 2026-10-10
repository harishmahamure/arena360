//! Real process/SQLite crashes with an independent HTTP S3 protocol stand-in.
use gaming_cafe_api::{
    background::{BackgroundJobs, Limits as JobLimits},
    control::{LeaseClient, LeaseConfig},
    metrics::Metrics,
    replication::{
        crypto::TenantKeys,
        ledger::PostgresLedger,
        recovery::Recoverer,
        snapshot, wal,
        worker::{self, Worker},
    },
    tenancy::{
        tenant_path, write_outbox_event_on_connection, NewOutboxEvent, TenantDbConfig,
        TenantDbManager,
    },
};
use object_store::ObjectStore;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Connection, SqliteConnection,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;
#[derive(Default)]
struct S3 {
    objects: tokio::sync::Mutex<HashMap<String, (bytes::Bytes, String)>>,
}
async fn s3_request(
    axum::extract::State(state): axum::extract::State<Arc<S3>>,
    request: axum::extract::Request,
) -> axum::response::Response {
    use axum::{
        body::Body,
        http::{Method, StatusCode},
    };
    let path = request.uri().path().to_owned();
    let method = request.method().clone();
    let headers = request.headers().clone();
    let data = match axum::body::to_bytes(request.into_body(), 512 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return axum::response::Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::empty())
                .unwrap()
        }
    };
    let mut objects = state.objects.lock().await;
    let error = |status, code: &str| {
        axum::response::Response::builder()
            .status(status)
            .header("content-type", "application/xml")
            .body(Body::from(format!(
                "<Error><Code>{code}</Code><Message>Fixture</Message></Error>"
            )))
            .unwrap()
    };
    match method {
        Method::PUT => {
            if headers.get("if-none-match").is_some() && objects.contains_key(&path) {
                return error(StatusCode::PRECONDITION_FAILED, "PreconditionFailed");
            }
            if let Some(expected) = headers.get("if-match") {
                if objects
                    .get(&path)
                    .is_none_or(|(_, etag)| expected.to_str().ok() != Some(etag.as_str()))
                {
                    return error(StatusCode::PRECONDITION_FAILED, "PreconditionFailed");
                }
            }
            let etag = format!("\"{}\"", wal::checksum(&data));
            objects.insert(path, (data, etag.clone()));
            axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("etag", etag)
                .body(Body::empty())
                .unwrap()
        }
        Method::GET | Method::HEAD => {
            let Some((bytes, etag)) = objects.get(&path) else {
                return error(StatusCode::NOT_FOUND, "NoSuchKey");
            };
            axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("etag", etag)
                .header(
                    "last-modified",
                    chrono::Utc::now()
                        .format("%a, %d %b %Y %H:%M:%S GMT")
                        .to_string(),
                )
                .header("content-length", bytes.len())
                .body(if method == Method::HEAD {
                    Body::empty()
                } else {
                    Body::from(bytes.clone())
                })
                .unwrap()
        }
        Method::DELETE => {
            objects.remove(&path);
            axum::response::Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(Body::empty())
                .unwrap()
        }
        _ => error(StatusCode::BAD_REQUEST, "InvalidRequest"),
    }
}
fn store(endpoint: &str) -> Arc<dyn ObjectStore> {
    Arc::new(
        object_store::aws::AmazonS3Builder::new()
            .with_bucket_name("recovery-fixture")
            .with_region("us-east-1")
            .with_endpoint(endpoint)
            .with_virtual_hosted_style_request(false)
            .with_allow_http(true)
            .with_access_key_id("fixture")
            .with_secret_access_key("fixture")
            .with_retry(object_store::RetryConfig {
                max_retries: 0,
                retry_timeout: Duration::from_secs(5),
                ..Default::default()
            })
            .build()
            .unwrap(),
    )
}
async fn pool() -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&std::env::var("CONTROL_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap()
}
async fn source_file(root: &Path, tenant: Uuid) {
    let path = tenant_path(root, tenant);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    gaming_cafe_api::tenancy::migrate(&pool).await.unwrap();
    sqlx::query("CREATE TABLE crash_commits(number INTEGER PRIMARY KEY,event_sequence INTEGER NOT NULL,committed_at TEXT NOT NULL,padding BLOB NOT NULL)").execute(&pool).await.unwrap();
    pool.close().await;
}
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Ack {
    number: i64,
    event_sequence: i64,
    at: chrono::DateTime<chrono::Utc>,
}
/// Only the parent invokes this ignored helper in a separate test executable.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "child process helper"]
async fn crash_driver_child() {
    let Ok(root) = std::env::var("ARENA_RECOVERY_DRIVER_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("arena360-cell-crash-"));
    let tenant = std::env::var("ARENA_RECOVERY_DRIVER_TENANT")
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    let cell = std::env::var("ARENA_RECOVERY_DRIVER_CELL")
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    let endpoint = std::env::var("ARENA_RECOVERY_DRIVER_S3").unwrap();
    let pool = pool().await;
    let leases = Arc::new(LeaseClient::new(pool.clone(), cell, LeaseConfig::default()).unwrap());
    leases.acquire_assigned(tenant).await.unwrap();
    leases.clone().spawn_renewal();
    let manager = Arc::new(
        TenantDbManager::new(
            TenantDbConfig {
                root: root.join("source"),
                ..Default::default()
            },
            leases,
        )
        .unwrap()
        .with_background_jobs(BackgroundJobs::new(JobLimits::default()).unwrap()),
    );
    let db = manager.open(tenant).await.unwrap();
    let keys = TenantKeys::new(root.join("separate-keys"));
    let store = store(&endpoint);
    let ledger = Arc::new(PostgresLedger {
        pool,
        cell_id: cell,
    });
    snapshot::take(
        db.clone(),
        &ledger,
        store.as_ref(),
        &keys,
        snapshot::Kind::Baseline,
    )
    .await
    .unwrap();
    let metrics = Arc::new(Metrics::default());
    let worker = Arc::new(Worker {
        gates: Default::default(),
        store,
        ledger,
        keys,
        metrics: metrics.clone(),
    });
    worker::spawn_capture(manager.clone(), metrics, root.join("source"));
    worker::spawn_upload(manager, worker);
    for number in 1..1_000_000i64 {
        let ack = db
            .with_immediate_writer(move |c| {
                Box::pin(async move {
                    let at = gaming_cafe_api::time::format_sqlite_timestamp(&chrono::Utc::now())
                        .unwrap();
                    sqlx::query("INSERT INTO crash_commits VALUES(?,0,?,?)")
                        .bind(number)
                        .bind(at)
                        .bind(vec![number as u8; 4096])
                        .execute(&mut *c)
                        .await?;
                    let event = write_outbox_event_on_connection(
                        c,
                        NewOutboxEvent {
                            location_id: None,
                            aggregate_type: "crash_fixture".into(),
                            aggregate_id: Uuid::new_v4(),
                            event_type: "fixture.committed".into(),
                            schema_version: 1,
                            deleted: false,
                            payload: serde_json::json!({"number":number}),
                        },
                    )
                    .await?;
                    sqlx::query("UPDATE crash_commits SET event_sequence=? WHERE number=?")
                        .bind(event.sequence)
                        .bind(number)
                        .execute(c)
                        .await?;
                    Ok(Ack {
                        number,
                        event_sequence: event.sequence,
                        at: chrono::Utc::now(),
                    })
                })
            })
            .await
            .unwrap();
        if number % 100 == 0 {
            let temp = root.join(format!("ack-{}.tmp", Uuid::new_v4()));
            wal::durable_create(&temp, &serde_json::to_vec(&ack).unwrap()).unwrap();
            std::fs::rename(temp, root.join("ack.json")).unwrap();
            std::fs::File::open(&root).unwrap().sync_all().unwrap();
        }
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires an isolated control-plane database"]
async fn sigkill_preserves_local_commits_and_disk_loss_recovers_verified_uploads() {
    let pool = pool().await;
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    let root = std::env::temp_dir().join(format!("arena360-cell-crash-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let tenant = Uuid::new_v4();
    let old = Uuid::new_v4();
    let target = Uuid::new_v4();
    for cell in [old, target] {
        sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)")
            .bind(cell)
            .bind(format!("process-crash-{cell}"))
            .bind(format!("http://{cell}.invalid"))
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,ownership_generation,state,schema_version) VALUES($1,$2,'Process crash','UTC',$3,1,'ACTIVE',$4)").bind(tenant).bind(format!("crash-{tenant}")).bind(old).bind(gaming_cafe_api::tenancy::target_schema_version()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tenant_leases(tenant_id,owner_cell,ownership_generation,expires_at) VALUES($1,$2,1,NOW()+INTERVAL '5 minutes')").bind(tenant).bind(old).execute(&pool).await.unwrap();
    source_file(&root.join("source"), tenant).await;
    let keys = TenantKeys::new(root.join("separate-keys"));
    wal::durable_create(&keys.path(tenant), &[42u8; 32]).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            axum::Router::new()
                .fallback(s3_request)
                .with_state(Arc::new(S3::default())),
        )
        .await
        .unwrap();
    });
    let log = std::fs::File::create(root.join("child.log")).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_driver_child", "--ignored", "--nocapture"])
        .env("ARENA_RECOVERY_DRIVER_ROOT", &root)
        .env("ARENA_RECOVERY_DRIVER_TENANT", tenant.to_string())
        .env("ARENA_RECOVERY_DRIVER_CELL", old.to_string())
        .env("ARENA_RECOVERY_DRIVER_S3", &endpoint)
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let ready=tokio::time::timeout(Duration::from_secs(75),async {
        loop {
            if let Some(status)=child.try_wait().unwrap(){panic!("Child ended {status}: {}",std::fs::read_to_string(root.join("child.log")).unwrap());}
            let verified:i64=sqlx::query_scalar("SELECT COUNT(*) FROM replication_segments s JOIN replication_generations g ON g.id=s.generation_id WHERE g.tenant_id=$1 AND s.verified_at IS NOT NULL").bind(tenant).fetch_one(&pool).await.unwrap();
            let ack=std::fs::read(root.join("ack.json")).ok().and_then(|b|serde_json::from_slice::<Ack>(&b).ok());
            if verified>=2 && ack.as_ref().is_some_and(|a|a.number>=2000) {break;}
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }).await;
    child.kill().unwrap();
    let status = child.wait().unwrap();
    assert!(!status.success());
    assert!(
        ready.is_ok(),
        "No verified uploads under traffic: {}",
        std::fs::read_to_string(root.join("child.log")).unwrap()
    );
    let ack: Ack = serde_json::from_slice(&std::fs::read(root.join("ack.json")).unwrap()).unwrap();
    let path = tenant_path(&root.join("source"), tenant);
    let mut local = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
        .await
        .unwrap();
    let local_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM crash_commits")
        .fetch_one(&mut local)
        .await
        .unwrap();
    let outbox: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&mut local)
        .await
        .unwrap();
    assert!(local_count >= ack.number);
    assert_eq!(outbox, local_count);
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&mut local)
            .await
            .unwrap(),
        "ok"
    );
    local.close().await.unwrap();
    assert!(
        std::fs::read_dir(path.parent().unwrap().join("replication/spool"))
            .unwrap()
            .count()
            > 0,
        "Spool must survive the killed process"
    );
    std::fs::remove_dir_all(root.join("source")).unwrap();
    sqlx::query("UPDATE tenant_leases SET renewed_at=NOW()-INTERVAL '6 minutes',expires_at=NOW()-INTERVAL '60 seconds' WHERE tenant_id=$1").bind(tenant).execute(&pool).await.unwrap();
    let leases = Arc::new(LeaseClient::new(pool.clone(), target, LeaseConfig::default()).unwrap());
    let manager = Arc::new(
        TenantDbManager::new(
            TenantDbConfig {
                root: root.join("replacement"),
                ..Default::default()
            },
            leases.clone(),
        )
        .unwrap(),
    );
    let recoverer = Arc::new(Recoverer {
        ledger: Arc::new(PostgresLedger {
            pool: pool.clone(),
            cell_id: target,
        }),
        leases,
        databases: manager.clone(),
        store: store(&endpoint),
        keys,
        staging_root: root.join("restore-staging"),
        analytics: None,
    });
    let outcome = recoverer.recover_tenant(tenant).await.unwrap();
    assert!(outcome.operations_ready);
    let db = manager.open(tenant).await.unwrap();
    let remote_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM crash_commits")
        .fetch_one(&db.read_pool().unwrap())
        .await
        .unwrap();
    let remote_outbox: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM outbox_events WHERE aggregate_type='crash_fixture'",
    )
    .fetch_one(&db.read_pool().unwrap())
    .await
    .unwrap();
    assert!(remote_count > 0 && remote_count <= local_count);
    assert_eq!(remote_outbox, remote_count);
    let recovered: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT recovered_at FROM tenant_recovery_jobs WHERE tenant_id=$1")
            .bind(tenant)
            .fetch_one(&pool)
            .await
            .unwrap();
    let loss_millis = (ack.at - recovered).num_milliseconds().max(0);
    assert!(loss_millis <= 120_000, "Recovery point lag {loss_millis}ms");
    println!("Process crash evidence: acknowledged={}, locally recovered={}, restored={}, recovery-point lag={}ms",ack.number,local_count,remote_count,loss_millis);
    db.close().await.unwrap();
    server.abort();
    sqlx::query("DELETE FROM tenants WHERE id=$1")
        .bind(tenant)
        .execute(&pool)
        .await
        .unwrap();
    for cell in [old, target] {
        sqlx::query("DELETE FROM cells WHERE id=$1")
            .bind(cell)
            .execute(&pool)
            .await
            .unwrap();
    }
    pool.close().await;
    std::fs::remove_dir_all(root).unwrap();
}

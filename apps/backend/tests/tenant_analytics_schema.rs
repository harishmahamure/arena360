#![cfg(feature = "duckdb-analytics")]
mod support;
use gaming_cafe_api::analytics::tenant_db::{error, TenantAnalytics};
use support::TenantFixture;
#[tokio::test]
async fn schema_is_tenant_local_exact_and_generated_values_remain_consistent() {
    let a = TenantFixture::new().await;
    let b = TenantFixture::new().await;
    let aa = TenantAnalytics::open(a.db.clone()).await.unwrap();
    let bb = TenantAnalytics::open(b.db.clone()).await.unwrap();
    assert_ne!(aa.path(), bb.path());
    assert_eq!(aa.tenant_id(), a.db.tenant_id());
    aa.write(|c| {
  c.execute_batch("INSERT INTO transactions(id,occurred_at,local_date,transaction_type,payment_method,payment_status,amount,paid_amount) VALUES('00000000-0000-0000-0000-000000000001',TIMESTAMP '2026-10-01 00:00:00',DATE '2026-10-01','plan_purchase','cash','completed',922337203685477.5807,922337203685477.5807)").map_err(error)?;
  let (money,booked):(String,bool)=c.query_row("SELECT CAST(amount AS VARCHAR),is_booked FROM transactions",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(error)?;assert_eq!(money,"922337203685477.5807");assert!(booked);
  c.execute_batch("INSERT INTO transaction_lines VALUES('00000000-0000-0000-0000-000000000002','00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000003',3,12.3456)").map_err(error)?;
  let total:String=c.query_row("SELECT CAST(line_total AS VARCHAR) FROM transaction_lines",[],|r|r.get(0)).map_err(error)?;assert_eq!(total,"37.0368");Ok(())
 }).await.unwrap();
    assert_eq!(
        bb.read(|c| c
            .query_row("SELECT COUNT(*) FROM transactions", [], |r| r
                .get::<_, i64>(0))
            .map_err(error))
            .await
            .unwrap(),
        0
    );
    let (tables, state): (i64, String) = aa
        .read(|c| {
            Ok((
                c.query_row(
                    "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='main'",
                    [],
                    |r| r.get(0),
                )
                .map_err(error)?,
                c.query_row("SELECT status FROM _ingest_state", [], |r| r.get(0))
                    .map_err(error)?,
            ))
        })
        .await
        .unwrap();
    assert_eq!(tables, 31);
    assert_eq!(state, "REBUILDING");
    drop(aa);
    drop(bb);
    a.close().await;
    b.close().await;
}
#[tokio::test]
async fn repeated_migrations_preserve_facts_and_checkpoints_and_reject_changed_history() {
    let f = TenantFixture::new().await;
    let analytics = TenantAnalytics::open(f.db.clone()).await.unwrap();
    analytics
        .write(|c| {
            c.execute_batch("UPDATE _ingest_state SET last_sequence=42,status='READY'")
                .map_err(error)?;
            Ok(())
        })
        .await
        .unwrap();
    drop(analytics);
    let analytics = TenantAnalytics::open(f.db.clone()).await.unwrap();
    assert_eq!(
        analytics
            .read(|c| c
                .query_row("SELECT last_sequence FROM _ingest_state", [], |r| r
                    .get::<_, u64>(0))
                .map_err(error))
            .await
            .unwrap(),
        42
    );
    analytics
        .write(|c| {
            c.execute_batch("UPDATE _schema_migrations SET checksum='tampered'")
                .map_err(error)?;
            Ok(())
        })
        .await
        .unwrap();
    drop(analytics);
    assert!(TenantAnalytics::open(f.db.clone()).await.is_err());
    f.close().await;
}
#[tokio::test]
async fn reads_rollback_mutations_and_fencing_rolls_back_writes() {
    let f = std::sync::Arc::new(TenantFixture::new().await);
    let analytics = TenantAnalytics::open(f.db.clone()).await.unwrap();
    analytics
        .read(|c| {
            c.execute_batch("UPDATE _ingest_state SET last_sequence=12")
                .map_err(error)?;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(
        analytics
            .read(|c| c
                .query_row("SELECT last_sequence FROM _ingest_state", [], |r| r
                    .get::<_, u64>(0))
                .map_err(error))
            .await
            .unwrap(),
        0
    );
    let fence = f.clone();
    assert!(analytics
        .write(move |c| {
            c.execute_batch("UPDATE _ingest_state SET last_sequence=99")
                .map_err(error)?;
            fence.revoke();
            Ok(())
        })
        .await
        .is_err());
    assert!(analytics.read(|_| Ok(())).await.is_err());
    assert!(TenantAnalytics::open(f.db.clone()).await.is_err());
    drop(analytics);
    let path = f.db.path().with_file_name("analytics.duckdb");
    let c = duckdb::Connection::open(path).unwrap();
    assert_eq!(
        c.query_row("SELECT last_sequence FROM _ingest_state", [], |r| r
            .get::<_, u64>(0))
            .unwrap(),
        0
    );
    drop(c);
    std::sync::Arc::try_unwrap(f).ok().unwrap().close().await;
}
#[tokio::test]
async fn newer_schema_and_timezone_drift_fail_without_rewriting_state() {
    let f = TenantFixture::new().await;
    let analytics = TenantAnalytics::open(f.db.clone()).await.unwrap();
    analytics
        .write(|c| {
            c.execute_batch("INSERT INTO _schema_migrations VALUES(3,'unknown',CURRENT_TIMESTAMP)")
                .map_err(error)?;
            Ok(())
        })
        .await
        .unwrap();
    drop(analytics);
    assert!(TenantAnalytics::open(f.db.clone()).await.is_err());
    let path = f.db.path().with_file_name("analytics.duckdb");
    let c = duckdb::Connection::open(&path).unwrap();
    c.execute_batch("DELETE FROM _schema_migrations WHERE version=3; UPDATE _ingest_state SET timezone='America/New_York'").unwrap();
    drop(c);
    assert!(TenantAnalytics::open(f.db.clone()).await.is_err());
    let c = duckdb::Connection::open(path).unwrap();
    assert_eq!(
        c.query_row("SELECT timezone FROM _ingest_state", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "America/New_York"
    );
    drop(c);
    f.close().await;
}

#[tokio::test]
async fn migration_uses_tenant_calendar_and_rejects_a_foreign_analytics_file() {
    use gaming_cafe_api::analytics::tenant_db::migrate;
    let a = TenantFixture::new().await;
    let b = TenantFixture::new().await;
    let now = chrono::DateTime::parse_from_rfc3339("2026-10-01T00:10:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let mut c = duckdb::Connection::open(a.db.path().with_file_name("analytics.duckdb")).unwrap();
    assert!(migrate(&mut c, &b.db, "America/New_York", now).is_err());
    migrate(&mut c, &a.db, "America/New_York", now).unwrap();
    let start: String = c
        .query_row(
            "SELECT CAST(hot_window_start AS VARCHAR) FROM _ingest_state",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(start, "2025-03-01");
    drop(c);
    a.close().await;
    b.close().await;
}
#[tokio::test]
async fn failed_ddl_rolls_back_every_new_table_and_migration_record() {
    let f = TenantFixture::new().await;
    let path = f.db.path().with_file_name("analytics.duckdb");
    let c = duckdb::Connection::open(&path).unwrap();
    c.execute_batch("CREATE TABLE users(id INTEGER)").unwrap();
    drop(c);
    assert!(TenantAnalytics::open(f.db.clone()).await.is_err());
    let c = duckdb::Connection::open(path).unwrap();
    assert_eq!(c.query_row("SELECT COUNT(*) FROM information_schema.tables WHERE table_name IN('_schema_migrations','_ingest_state','transactions')",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    drop(c);
    f.close().await;
}

#[tokio::test]
async fn v1_upgrade_preserves_facts_and_checkpoint_but_requires_a_rebuild() {
    use gaming_cafe_api::analytics::tenant_db::{migrate, SCHEMA_VERSION};
    use sha2::{Digest, Sha256};
    let f = TenantFixture::new().await;
    let mut c = duckdb::Connection::open(f.db.path().with_file_name("analytics.duckdb")).unwrap();
    let v1 = include_str!("../migrations/analytics/0001_initial.sql");
    c.execute_batch(v1).unwrap();
    c.execute_batch("CREATE TABLE _schema_migrations(version INTEGER PRIMARY KEY,checksum VARCHAR NOT NULL,applied_at TIMESTAMP NOT NULL); INSERT INTO _ingest_state(id,schema_version,status,last_sequence,hot_window_start,timezone,updated_at) VALUES(1,1,'READY',42,DATE '2025-04-01','UTC',current_timestamp); INSERT INTO transactions(id,occurred_at,local_date,transaction_type,payment_method,payment_status,amount,paid_amount) VALUES(uuid(),TIMESTAMP '2026-10-01',DATE '2026-10-01','product_purchase','cash','completed',12.3456,12.3456)").unwrap();
    c.execute(
        "INSERT INTO _schema_migrations VALUES(1,?,current_timestamp)",
        duckdb::params![hex::encode(Sha256::digest(v1.as_bytes()))],
    )
    .unwrap();
    migrate(&mut c, &f.db, "UTC", chrono::Utc::now()).unwrap();
    let state:(i64,String,i64,Option<u64>)=c.query_row("SELECT schema_version,status,CAST(last_sequence AS BIGINT),replay_start_sequence FROM _ingest_state",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
    assert_eq!(state, (SCHEMA_VERSION, "REBUILDING".into(), 42, None));
    assert_eq!(
        c.query_row(
            "SELECT CAST(amount AS VARCHAR) FROM transactions",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "12.3456"
    );
    migrate(&mut c, &f.db, "UTC", chrono::Utc::now()).unwrap();
    assert_eq!(
        c.query_row("SELECT count(*) FROM _schema_migrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    drop(c);
    f.close().await;
}

#![cfg(feature = "duckdb-analytics")]
mod support;
use gaming_cafe_api::{
    analytics::{
        rebuild::{rebuild, rebuild_with_hook, source_watermark},
        tenant_db::{error, TenantAnalytics},
    },
    error::AppError,
    metrics::Metrics,
    tenancy::{write_outbox_event_on_connection, NewOutboxEvent},
};
use serde_json::json;
use support::TenantFixture;
#[tokio::test]
async fn rebuild_watermark_survives_complete_outbox_cleanup() {
    let f = TenantFixture::new().await;
    f.player("watermark-player").await;
    let t0 = source_watermark(&f.db).await.unwrap();
    assert!(t0 > 0);
    f.db.with_immediate_writer(|c| {
        Box::pin(async move {
            sqlx::query("DELETE FROM outbox_events")
                .execute(&mut *c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert_eq!(source_watermark(&f.db).await.unwrap(), t0);
    f.close().await;
}
#[tokio::test]
#[ignore = "requires the matching signed SQLite extension"]
async fn live_rebuild_switches_only_after_snapshot_replay_and_preserves_failed_builds() {
    let jobs=gaming_cafe_api::background::BackgroundJobs::new(gaming_cafe_api::background::Limits::default()).unwrap();
    let f = TenantFixture::new_with_background_jobs(jobs).await;
    let player = f.player("snapshot-player").await;
    let a = TenantAnalytics::open(f.db.clone()).await.unwrap();
    a.write(|tx|{tx.execute_batch("INSERT INTO monthly_summary VALUES(DATE '2020-01-01','00000000-0000-0000-0000-000000000000',21.0101,20.0101,1.0000,2,3,4,5)").map_err(error)?;Ok(())}).await.unwrap();
    let metrics = Metrics::default();
    let t0 = source_watermark(&f.db).await.unwrap();
    let db = f.db.clone();
    let live = a.clone();
    rebuild_with_hook(f.db.clone(), a.clone(), || async {
        assert_eq!(
            live.read(|tx| tx
                .query_row("SELECT status FROM _ingest_state", [], |r| r
                    .get::<_, String>(0))
                .map_err(error))
                .await?,
            "REBUILDING"
        );
        // Foreground mutation after T0 must be replayed, not read from live tables.
        db.with_immediate_writer(move |c| {
            Box::pin(async move {
                sqlx::query("UPDATE users SET credit_limit=9223372036854775807 WHERE id=?")
                    .bind(player.to_string())
                    .execute(&mut *c)
                    .await?;
                write_outbox_event_on_connection(
                    c,
                    NewOutboxEvent {
                        aggregate_type: "user".into(),
                        aggregate_id: player,
                        event_type: "user.updated".into(),
                        location_id: None,
                        schema_version: 1,
                        deleted: false,
                        payload: json!({"id":player}),
                    },
                )
                .await?;
                Ok(())
            })
        })
        .await?;
        Ok(())
    })
    .await
    .unwrap();
    let values:(String,i64,i64,String)=a.read(|tx|Ok((tx.query_row("SELECT status FROM _ingest_state",[],|r|r.get(0)).map_err(error)?,tx.query_row("SELECT CAST(last_sequence AS BIGINT) FROM _ingest_state",[],|r|r.get(0)).map_err(error)?,tx.query_row("SELECT CAST(rebuild_boundary_seq AS BIGINT) FROM _ingest_state",[],|r|r.get(0)).map_err(error)?,tx.query_row("SELECT CAST(credit_limit AS VARCHAR) FROM users WHERE username='snapshot-player'",[],|r|r.get(0)).map_err(error)?))).await.unwrap();
    assert_eq!(
        values,
        ("READY".into(), t0 + 1, t0, "922337203685477.5807".into())
    );
    let mut consumer = gaming_cafe_api::analytics::consumer::LocalConsumer::connect(
        f.db.clone(),
        a.clone(),
    )
    .await
    .unwrap();
    let outcome = consumer.poll(&metrics).await.unwrap();
    assert!(
        matches!(outcome,gaming_cafe_api::analytics::consumer::BatchOutcome::Applied{last_sequence,events:0,..} if last_sequence==t0+1)
    );
    assert_eq!(
        a.read(|tx| tx
            .query_row(
                "SELECT CAST(credit_limit AS VARCHAR) FROM users WHERE username='snapshot-player'",
                [],
                |r| r.get::<_, String>(0)
            )
            .map_err(error))
            .await
            .unwrap(),
        "922337203685477.5807"
    );
    drop(consumer);
    assert!(
        rebuild_with_hook(f.db.clone(), a.clone(), || async {
            Err(AppError::Internal("injected snapshot failure".into()))
        })
        .await
        .is_err()
    );
    assert_eq!(
        a.read(|tx| tx
            .query_row("SELECT status FROM _ingest_state", [], |r| r
                .get::<_, String>(0))
            .map_err(error))
            .await
            .unwrap(),
        "REBUILDING"
    );
    assert_eq!(
        a.read(|tx| tx
            .query_row(
                "SELECT CAST(credit_limit AS VARCHAR) FROM users WHERE username='snapshot-player'",
                [],
                |r| r.get::<_, String>(0)
            )
            .map_err(error))
            .await
            .unwrap(),
        "922337203685477.5807"
    );
    rebuild(f.db.clone(), a.clone()).await.unwrap();
    // A second rebuild starts from the persistent watermark even if rows were purged.
    f.db.with_immediate_writer(|c| {
        Box::pin(async move {
            sqlx::query("DELETE FROM outbox_events")
                .execute(&mut *c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    rebuild(f.db.clone(), a.clone()).await.unwrap();
    assert_eq!(
        a.read(|tx| tx
            .query_row(
                "SELECT CAST(last_sequence AS BIGINT) FROM _ingest_state",
                [],
                |r| r.get::<_, i64>(0)
            )
            .map_err(error))
            .await
            .unwrap(),
        t0 + 1
    );
    assert_eq!(
        std::fs::read_dir(f.db.path().parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("rebuild-"))
            .count(),
        0
    );
    assert_eq!(a.read(|tx|tx.query_row("SELECT CAST(revenue AS VARCHAR) FROM monthly_summary WHERE month=DATE '2020-01-01'",[],|r|r.get::<_,String>(0)).map_err(error)).await.unwrap(),"21.0101");
    // Self-fencing after T0 prevents either backfill or the file switch.
    assert!(
        rebuild_with_hook(f.db.clone(), a.clone(), || async {
            f.revoke();
            Ok(())
        })
        .await
        .is_err()
    );
    drop(a);
    f.close().await;
}

#[tokio::test]
#[ignore = "requires the matching signed SQLite extension"]
async fn local_replay_gap_preserves_live_facts_and_recovers_from_a_new_snapshot() {
    let f = TenantFixture::new().await;
    let player = f.player("retention-gap-player").await;
    let analytics = TenantAnalytics::open(f.db.clone()).await.unwrap();
    rebuild(f.db.clone(), analytics.clone()).await.unwrap();
    let db = f.db.clone();
    assert!(rebuild_with_hook(f.db.clone(), analytics.clone(), || async move {
        db.with_immediate_writer(move |c| Box::pin(async move {
            sqlx::query("UPDATE users SET credit_limit=10000 WHERE id=?")
                .bind(player.to_string()).execute(&mut *c).await?;
            write_outbox_event_on_connection(c, NewOutboxEvent {
                aggregate_type: "user".into(), aggregate_id: player,
                event_type: "user.updated".into(), location_id: None,
                schema_version: 1, deleted: false, payload: json!({"id":player}),
            }).await?;
            // Simulate retention racing the snapshot backfill.
            sqlx::query("DELETE FROM outbox_events").execute(c).await?;
            Ok(())
        })).await
    }).await.is_err());
    let stored = analytics.read(|tx| tx.query_row(
        "SELECT CAST(credit_limit AS VARCHAR),(SELECT status FROM _ingest_state) FROM users WHERE username='retention-gap-player'",
        [], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    ).map_err(error)).await.unwrap();
    assert_eq!(stored, ("0.0000".into(), "REBUILDING".into()));
    rebuild(f.db.clone(), analytics.clone()).await.unwrap();
    let stored = analytics.read(|tx| tx.query_row(
        "SELECT CAST(credit_limit AS VARCHAR),(SELECT status FROM _ingest_state) FROM users WHERE username='retention-gap-player'",
        [], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    ).map_err(error)).await.unwrap();
    assert_eq!(stored, ("1.0000".into(), "READY".into()));
    drop(analytics);
    f.close().await;
}

#[tokio::test]
async fn corrupt_derived_file_is_quarantined_without_changing_operational_data() {
    let f = TenantFixture::new().await;
    f.player("corruption-player").await;
    let a = TenantAnalytics::open(f.db.clone()).await.unwrap();
    let path = a.path().to_owned();
    drop(a);
    std::fs::write(&path, b"corrupt derived storage").unwrap();
    assert!(TenantAnalytics::open(f.db.clone()).await.is_err());
    let a = TenantAnalytics::open_for_ingestion(f.db.clone())
        .await
        .unwrap();
    assert_eq!(
        a.read(|tx| tx
            .query_row("SELECT status FROM _ingest_state", [], |r| r
                .get::<_, String>(0))
            .map_err(error))
            .await
            .unwrap(),
        "REBUILDING"
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM users WHERE username='corruption-player'")
            .fetch_one(&f.db.read_pool().unwrap())
            .await
            .unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e
                .file_name()
                .to_string_lossy()
                .starts_with("analytics.quarantined-"))
            .count(),
        1
    );
    drop(a);
    f.close().await;
}
#[tokio::test]
async fn ingestion_rebuilds_changed_schema_history_but_preserves_newer_binary_files() {
    let f = TenantFixture::new().await;
    let a = TenantAnalytics::open(f.db.clone()).await.unwrap();
    a.write(|tx| {
        tx.execute_batch("DELETE FROM _schema_migrations WHERE version>1")
            .map_err(error)?;
        Ok(())
    })
    .await
    .unwrap();
    drop(a);
    assert!(TenantAnalytics::open(f.db.clone()).await.is_err());
    let a = TenantAnalytics::open_for_ingestion(f.db.clone())
        .await
        .unwrap();
    a.write(|tx| {
        tx.execute_batch(
            "INSERT INTO _schema_migrations VALUES(4,'newer-definition',current_timestamp)",
        )
        .map_err(error)?;
        Ok(())
    })
    .await
    .unwrap();
    drop(a);
    assert!(TenantAnalytics::open_for_ingestion(f.db.clone())
        .await
        .is_err());
    let c = duckdb::Connection::open(f.db.path().with_file_name("analytics.duckdb")).unwrap();
    assert_eq!(
        c.query_row("SELECT max(version) FROM _schema_migrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        4
    );
    drop(c);
    f.close().await;
}
#[tokio::test]
async fn monthly_summaries_keep_exact_revenue_distinct_visitors_and_old_months() {
    use duckdb::params;
    use gaming_cafe_api::analytics::{consumer::replace_hours, rebuild::refresh_monthly};
    let f = TenantFixture::new().await;
    let a = TenantAnalytics::open(f.db.clone()).await.unwrap();
    let p = uuid::Uuid::new_v4().to_string();
    let venue1 = uuid::Uuid::new_v4().to_string();
    let venue2 = uuid::Uuid::new_v4().to_string();
    a.write(move|tx| {
        tx.execute_batch("UPDATE _ingest_state SET hot_window_start=DATE '2025-01-01'; INSERT INTO monthly_summary VALUES(DATE '2020-01-01','00000000-0000-0000-0000-000000000000',21.0101,20.0101,1.0000,2,3,4,5)").map_err(error)?;
        for (venue,staff,amount,status) in [(Some(&venue1),false,"10.0001","completed"),(Some(&venue2),false,"10.0001","credit"),(Some(&venue1),true,"100.0000","refunded"),(None,true,"2.0001","completed")] {
            let session=uuid::Uuid::new_v4().to_string();
            tx.execute("INSERT INTO sessions(id,device_id,balance_id,player_id,is_staff_allowance,location_id,start_time,end_time,start_local_date) VALUES(CAST(? AS UUID),uuid(),uuid(),CAST(? AS UUID),?,CAST(? AS UUID),TIMESTAMP '2026-10-01 10:00:00',TIMESTAMP '2026-10-01 11:00:00',DATE '2026-10-01')",params![session,p,staff,venue]).map_err(error)?;
            replace_hours(tx,&session,chrono_tz::UTC)?;
            tx.execute("INSERT INTO transactions(id,occurred_at,local_date,location_id,transaction_type,payment_method,payment_status,amount,paid_amount) VALUES(uuid(),TIMESTAMP '2026-10-01 10:00:00',DATE '2026-10-01',CAST(? AS UUID),'product_purchase','cash',?,CAST(? AS DECIMAL(19,4)),CAST(? AS DECIMAL(19,4)))",params![venue,status,amount,amount]).map_err(error)?;
        }
        refresh_monthly(tx)?;refresh_monthly(tx)?;
        Ok(())
    }).await.unwrap();
    let totals:(String,i64,i64,f64,i64)=a.read(|tx|tx.query_row("SELECT CAST(revenue AS VARCHAR),transactions,session_starts,occupied_hours,visitors FROM monthly_summary WHERE month=DATE '2026-10-01' AND location_id='00000000-0000-0000-0000-000000000000'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(error)).await.unwrap();
    assert_eq!(totals, ("22.0003".into(), 3, 2, 2.0, 1));
    assert_eq!(
        a.read(|tx| tx
            .query_row("SELECT count(*) FROM monthly_summary", [], |r| r
                .get::<_, i64>(0))
            .map_err(error))
            .await
            .unwrap(),
        4
    );
    assert_eq!(a.read(|tx|tx.query_row("SELECT CAST(revenue AS VARCHAR) FROM monthly_summary WHERE month=DATE '2020-01-01'",[],|r|r.get::<_,String>(0)).map_err(error)).await.unwrap(),"21.0101");
    drop(a);
    f.close().await;
}

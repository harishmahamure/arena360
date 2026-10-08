#![cfg(feature = "duckdb-analytics")]
mod support;
use gaming_cafe_api::analytics::{
    consumer::replace_hours,
    retention::{run, RetentionOutcome},
    tenant_db::{error, TenantAnalytics},
};
use support::TenantFixture;
fn at(s: &str) -> chrono::DateTime<chrono::Utc> {
    s.parse().unwrap()
}
#[tokio::test]
async fn rollover_seals_summaries_and_purges_in_batches_without_losing_open_or_crossing_work() {
    let jobs = gaming_cafe_api::background::BackgroundJobs::new(
        gaming_cafe_api::background::Limits::default(),
    )
    .unwrap();
    let f = TenantFixture::new_with_background_jobs(jobs).await;
    // A fully ingested/purged source retains its AUTOINCREMENT watermark.
    f.db.with_immediate_writer(|c| {
        Box::pin(async move {
            sqlx::query("INSERT INTO sqlite_sequence(name,seq) VALUES('outbox_events',42)")
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let a = TenantAnalytics::open(f.db.clone()).await.unwrap();
    a.write(|tx|{
  tx.execute_batch("UPDATE _ingest_state SET status='READY',last_sequence=42,hot_window_start=DATE '2025-03-01';
   INSERT INTO transactions(id,occurred_at,local_date,transaction_type,payment_method,payment_status,amount,paid_amount)
   SELECT uuid(),TIMESTAMP '2025-03-31 23:00:00',DATE '2025-03-31','product_purchase','cash','completed',1.0001,1.0001 FROM range(1001);
   INSERT INTO transaction_lines(id,transaction_id,product_id,quantity,unit_price) SELECT uuid(),id,uuid(),1,1.0001 FROM transactions;
   INSERT INTO transactions(id,occurred_at,local_date,transaction_type,payment_method,payment_status,amount,paid_amount) VALUES(uuid(),TIMESTAMP '2025-04-01',DATE '2025-04-01','plan_purchase','cash','completed',2.0002,2.0002);
   INSERT INTO monthly_summary VALUES(DATE '2020-01-01','00000000-0000-0000-0000-000000000000',99.0001,0,99.0001,1,0,0,0);
   INSERT INTO sessions(id,device_id,balance_id,player_id,is_staff_allowance,start_time,end_time,start_local_date)
   VALUES ('00000000-0000-0000-0000-000000000001',uuid(),uuid(),uuid(),false,TIMESTAMP '2025-03-31 23:00:00',TIMESTAMP '2025-04-01 01:00:00',DATE '2025-03-31'),
   ('00000000-0000-0000-0000-000000000002',uuid(),uuid(),uuid(),false,TIMESTAMP '2025-03-31 22:00:00',TIMESTAMP '2025-03-31 23:00:00',DATE '2025-03-31'),
   ('00000000-0000-0000-0000-000000000003',uuid(),uuid(),uuid(),false,TIMESTAMP '2025-03-31 23:00:00',NULL,DATE '2025-03-31');
   INSERT INTO shifts(id,user_id,clock_in,clock_out,status) VALUES(uuid(),uuid(),TIMESTAMP '2025-03-31',NULL,'open'),(uuid(),uuid(),TIMESTAMP '2025-03-31',TIMESTAMP '2025-04-01 01:00:00','closed'),(uuid(),uuid(),TIMESTAMP '2025-03-31',TIMESTAMP '2025-04-01','closed');
   INSERT INTO stock_waste_events VALUES(uuid(),uuid(),'pending',NULL),(uuid(),uuid(),'approved',TIMESTAMP '2025-03-31');
   INSERT INTO stock_waste_lines SELECT uuid(),id,uuid(),'damaged',1 FROM stock_waste_events;").map_err(error)?;
  for id in ["00000000-0000-0000-0000-000000000001","00000000-0000-0000-0000-000000000002"]{replace_hours(tx,id,chrono_tz::UTC)?;}
  Ok(())
 }).await.unwrap();
    let outcome = run(a.clone(), at("2026-10-01T00:00:00Z")).await.unwrap();
    assert!(
        matches!(outcome,RetentionOutcome::Applied{deleted_rows:2008,hot_window_start,next_due} if hot_window_start.to_string()=="2025-04-01" && next_due==at("2026-10-02T00:00:00Z"))
    );
    let counts = a
        .read(|tx| {
            let mut result = vec![];
            for table in [
                "transactions",
                "transaction_lines",
                "sessions",
                "session_hours",
                "shifts",
                "stock_waste_events",
                "stock_waste_lines",
            ] {
                result.push(
                    tx.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| {
                        r.get::<_, i64>(0)
                    })
                    .map_err(error)?,
                );
            }
            Ok(result)
        })
        .await
        .unwrap();
    assert_eq!(counts, vec![1, 0, 2, 1, 2, 1, 1]);
    let summary=a.read(|tx|tx.query_row("SELECT CAST(revenue AS VARCHAR),session_starts,occupied_hours,visitors FROM monthly_summary WHERE month=DATE '2025-03-01'",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,f64>(2)?,r.get::<_,i64>(3)?))).map_err(error)).await.unwrap();
    assert_eq!(summary, ("1001.1001".into(), 3, 2.0, 3));
    // Closing a crossing session must regenerate only retained hours.
    a.write(|tx| replace_hours(tx, "00000000-0000-0000-0000-000000000001", chrono_tz::UTC))
        .await
        .unwrap();
    assert!(matches!(
        run(a.clone(), at("2026-10-02T00:00:00Z")).await.unwrap(),
        RetentionOutcome::Applied {
            deleted_rows: 0,
            ..
        }
    ));
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
        42
    );
    assert_eq!(a.read(|tx|tx.query_row("SELECT CAST(revenue AS VARCHAR) FROM monthly_summary WHERE month=DATE '2025-03-01'",[],|r|r.get::<_,String>(0)).map_err(error)).await.unwrap(),"1001.1001");
    drop(a);
    f.close().await;
}
#[tokio::test]
async fn retention_skips_unready_state_and_respects_self_fencing() {
    let f = TenantFixture::new().await;
    let a = TenantAnalytics::open(f.db.clone()).await.unwrap();
    for status in ["REBUILDING", "LAGGING", "FAILED"] {
        a.write(move |tx| {
            tx.execute("UPDATE _ingest_state SET status=?", [status])
                .map_err(error)?;
            Ok(())
        })
        .await
        .unwrap();
        assert_eq!(
            run(a.clone(), at("2026-10-01T00:00:00Z")).await.unwrap(),
            RetentionOutcome::Skipped
        );
    }
    f.revoke();
    assert!(run(a.clone(), at("2026-10-01T00:00:00Z")).await.is_err());
    drop(a);
    f.close().await;
}
#[tokio::test]
async fn schedule_uses_tenant_month_and_next_local_midnight() {
    let f = TenantFixture::new().await;
    f.db.with_immediate_writer(|c| {
        Box::pin(async move {
            sqlx::query("UPDATE tenant_runtime SET timezone='Asia/Kolkata'")
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let a = TenantAnalytics::open(f.db.clone()).await.unwrap();
    a.write(|tx| {
        tx.execute_batch(
            "UPDATE _ingest_state SET status='READY',hot_window_start=DATE '2025-03-01'",
        )
        .map_err(error)?;
        Ok(())
    })
    .await
    .unwrap();
    assert!(
        matches!(run(a.clone(),at("2026-09-30T18:30:00Z")).await.unwrap(),RetentionOutcome::Applied{hot_window_start,next_due,..} if hot_window_start.to_string()=="2025-04-01" && next_due==at("2026-10-01T18:30:00Z"))
    );
    drop(a);
    f.close().await;
}

#[tokio::test]
async fn retention_waits_for_the_source_watermark_before_sealing() {
    let f = TenantFixture::new().await;
    f.player("pending-retention-source").await;
    let a = TenantAnalytics::open(f.db.clone()).await.unwrap();
    a.write(|tx| {
        tx.execute_batch(
            "UPDATE _ingest_state SET status='READY',hot_window_start=DATE '2025-03-01'",
        )
        .map_err(error)?;
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(
        run(a.clone(), at("2026-10-01T00:00:00Z")).await.unwrap(),
        RetentionOutcome::Skipped
    );
    assert_eq!(
        a.read(|tx| tx
            .query_row(
                "SELECT CAST(hot_window_start AS VARCHAR) FROM _ingest_state",
                [],
                |r| r.get::<_, String>(0)
            )
            .map_err(error))
            .await
            .unwrap(),
        "2025-03-01"
    );
    a.write(|tx| {
        tx.execute_batch("UPDATE _ingest_state SET last_sequence=1")
            .map_err(error)?;
        Ok(())
    })
    .await
    .unwrap();
    assert!(matches!(
        run(a.clone(), at("2026-10-01T00:00:00Z")).await.unwrap(),
        RetentionOutcome::Applied { .. }
    ));
    drop(a);
    f.close().await;
}
#[tokio::test]
async fn nightly_schedule_survives_a_twenty_five_hour_dst_day() {
    let f = TenantFixture::new().await;
    f.db.with_immediate_writer(|c| {
        Box::pin(async move {
            sqlx::query("UPDATE tenant_runtime SET timezone='Europe/Berlin'")
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let a = TenantAnalytics::open(f.db.clone()).await.unwrap();
    a.write(|tx| {
        tx.execute_batch("UPDATE _ingest_state SET status='READY'")
            .map_err(error)?;
        Ok(())
    })
    .await
    .unwrap();
    assert!(
        matches!(run(a.clone(),at("2026-10-24T22:00:00Z")).await.unwrap(),RetentionOutcome::Applied{next_due,..} if next_due==at("2026-10-25T23:00:00Z"))
    );
    drop(a);
    f.close().await;
}

#[tokio::test]
async fn retention_cannot_seal_facts_when_the_checkpoint_is_ahead_of_restored_sqlite() {
    let f = TenantFixture::new().await;
    let a = TenantAnalytics::open(f.db.clone()).await.unwrap();
    a.write(|tx|{tx.execute_batch("UPDATE _ingest_state SET status='READY',last_sequence=42,hot_window_start=DATE '2025-03-01'").map_err(error)?;Ok(())}).await.unwrap();
    assert_eq!(
        run(a.clone(), at("2026-10-01T00:00:00Z")).await.unwrap(),
        RetentionOutcome::Skipped
    );
    assert_eq!(
        a.read(|tx| tx
            .query_row(
                "SELECT CAST(hot_window_start AS VARCHAR) FROM _ingest_state",
                [],
                |r| r.get::<_, String>(0)
            )
            .map_err(error))
            .await
            .unwrap(),
        "2025-03-01"
    );
    drop(a);
    f.close().await;
}

#![cfg(feature = "duckdb-analytics")]
mod support;
use gaming_cafe_api::{
    analytics::{
        consumer::{apply_batch, BatchOutcome, LocalConsumer},
        events::TenantEvent,
        tenant_db::{error, TenantAnalytics},
    },
    tenancy::{
        analytics_snapshot::{capture, TABLES},
        write_outbox_event_on_connection, NewOutboxEvent,
    },
};
use serde_json::json;
use std::sync::Arc;
use support::{SessionFixture, TenantFixture};
use uuid::Uuid;
async fn ready(f: &TenantFixture) -> Arc<TenantAnalytics> {
    let a = TenantAnalytics::open(f.db.clone()).await.unwrap();
    a.write(|tx| {
        tx.execute_batch(
            "UPDATE _ingest_state SET status='READY',hot_window_start=DATE '2025-01-01'",
        )
        .map_err(error)?;
        Ok(())
    })
    .await
    .unwrap();
    a
}
async fn events(f: &TenantFixture) -> Vec<TenantEvent> {
    let mut events:Vec<TenantEvent>=sqlx::query_as("SELECT sequence,event_id,location_id,aggregate_type,aggregate_id,event_type,occurred_at,schema_version,deleted,payload,analytics_snapshot FROM outbox_events ORDER BY sequence").fetch_all(&f.db.read_pool().unwrap()).await.unwrap();
    for e in &mut events {
        e.tenant_id = f.db.tenant_id();
    }
    events
}
async fn checkpoint(a: &Arc<TenantAnalytics>) -> (i64, String) {
    a.read(|tx| {
        tx.query_row(
            "SELECT CAST(last_sequence AS BIGINT),status FROM _ingest_state",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(error)
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn every_projection_matches_sqlite_and_never_reads_credentials() {
    let f = TenantFixture::new().await;
    let p = f.player("projection-player").await;
    for spec in TABLES {
        let _: Vec<String> = sqlx::query_scalar(&spec.select())
            .fetch_all(&f.db.read_pool().unwrap())
            .await
            .unwrap();
        assert!(!spec.select().contains("password"));
    }
    let mut c = f.db.read_pool().unwrap().acquire().await.unwrap();
    let snapshot = capture(&mut c, "user", p, false).await.unwrap().unwrap();
    let text = serde_json::to_string(&snapshot).unwrap();
    assert!(!text.contains("password"));
    assert!(!text.contains("phone_number"));
    assert_eq!(snapshot.changes[0].rows[0]["credit_limit"], "0.0000");
    drop(c);
    f.close().await;
}
#[tokio::test]
async fn exact_snapshots_collapse_replay_and_tombstones_are_atomic() {
    let f = TenantFixture::new().await;
    let p = f.player("exact-player").await;
    let a = ready(&f).await;
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE users SET credit_limit=? WHERE id=?")
                .bind(i64::MAX)
                .bind(p.to_string())
                .execute(&mut *c)
                .await?;
            write_outbox_event_on_connection(
                c,
                NewOutboxEvent {
                    aggregate_type: "user".into(),
                    aggregate_id: p,
                    event_type: "user.updated".into(),
                    location_id: None,
                    schema_version: 1,
                    deleted: false,
                    payload: json!({"id":p,"creditLimit":0.0}),
                },
            )
            .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let batch = events(&f).await;
    assert_eq!(
        batch
            .last()
            .unwrap()
            .analytics_snapshot
            .as_ref()
            .unwrap()
            .changes[0]
            .rows[0]["credit_limit"],
        "922337203685477.5807"
    );
    let mut unordered = batch.clone();
    unordered.reverse();
    unordered.push(batch[0].clone());
    assert!(matches!(
        apply_batch(a.clone(), unordered).await.unwrap(),
        BatchOutcome::Applied {
            events: 2,
            duplicates: 1,
            ..
        }
    ));
    let stored: String = a
        .read(|tx| {
            tx.query_row("SELECT CAST(credit_limit AS VARCHAR) FROM users", [], |r| {
                r.get(0)
            })
            .map_err(error)
        })
        .await
        .unwrap();
    assert_eq!(stored, "922337203685477.5807");
    assert!(matches!(
        apply_batch(a.clone(), batch.clone()).await.unwrap(),
        BatchOutcome::Applied {
            events: 0,
            duplicates: 2,
            ..
        }
    ));
    let mut deleted = batch[1].clone();
    deleted.sequence = 3;
    deleted.event_id = Uuid::now_v7().to_string();
    deleted.deleted = true;
    let snapshot = deleted.analytics_snapshot.as_mut().unwrap();
    snapshot.changes[0].rows.clear();
    assert!(matches!(
        apply_batch(a.clone(), vec![deleted]).await.unwrap(),
        BatchOutcome::Applied {
            last_sequence: 3,
            ..
        }
    ));
    assert_eq!(
        a.read(|tx| tx
            .query_row("SELECT COUNT(*) FROM users", [], |r| r.get::<_, i64>(0))
            .map_err(error))
            .await
            .unwrap(),
        0
    );
    drop(a);
    f.close().await;
}
#[tokio::test]
async fn gap_and_foreign_or_invalid_batches_never_advance_or_partially_write() {
    let f = TenantFixture::new().await;
    let a = ready(&f).await;
    f.player("gap-player").await;
    let first = events(&f).await.remove(0);
    let mut third = first.clone();
    third.sequence = 3;
    third.event_id = Uuid::now_v7().to_string();
    assert_eq!(
        apply_batch(a.clone(), vec![first.clone(), third.clone()])
            .await
            .unwrap(),
        BatchOutcome::Gap {
            expected: 2,
            received: 3
        }
    );
    assert_eq!(checkpoint(&a).await, (0, "LAGGING".into()));
    assert_eq!(
        a.read(|tx| tx
            .query_row("SELECT COUNT(*) FROM users", [], |r| r.get::<_, i64>(0))
            .map_err(error))
            .await
            .unwrap(),
        0
    );
    let mut foreign = first.clone();
    foreign.tenant_id = Uuid::new_v4();
    assert!(apply_batch(a.clone(), vec![foreign]).await.is_err());
    let mut invalid = first.clone();
    invalid.analytics_snapshot.as_mut().unwrap().changes[0].rows[0]["credit_limit"] =
        json!("1.00001");
    assert!(apply_batch(a.clone(), vec![invalid]).await.is_err());
    assert_eq!(checkpoint(&a).await.0, 0);
    let mut second = first.clone();
    second.sequence = 2;
    second.event_id = Uuid::now_v7().to_string();
    assert!(matches!(
        apply_batch(a.clone(), vec![third, second, first])
            .await
            .unwrap(),
        BatchOutcome::Applied {
            last_sequence: 3,
            ..
        }
    ));
    let mut legacy = events(&f).await.remove(0);
    legacy.sequence = 4;
    legacy.analytics_snapshot.as_mut().unwrap().version = 1;
    assert_eq!(apply_batch(a.clone(), vec![legacy.clone()]).await.unwrap(), BatchOutcome::RebuildRequired);
    assert_eq!(checkpoint(&a).await, (3, "REBUILDING".into()));
    a.write(|tx| { tx.execute_batch("UPDATE _ingest_state SET status='READY'").map_err(error)?; Ok(()) }).await.unwrap();
    legacy.analytics_snapshot = None;
    assert_eq!(
        apply_batch(a.clone(), vec![legacy]).await.unwrap(),
        BatchOutcome::RebuildRequired
    );
    assert_eq!(checkpoint(&a).await, (3, "REBUILDING".into()));
    drop(a);
    f.close().await;
}
#[tokio::test]
async fn session_updates_replace_hours_and_preserve_start_attribution() {
    let s = SessionFixture::new().await;
    let a = ready(&s.tenant).await;
    let id = Uuid::now_v7();
    let device = s.device;
    let balance = s.balance;
    let player = s.player;
    let venue = s.venue;
    s.tenant.db.with_immediate_writer(move|c|Box::pin(async move{let start="2026-10-01T18:20:00.000000Z";let end="2026-10-01T19:10:00.000000Z";sqlx::query("UPDATE tenant_runtime SET timezone='Asia/Kolkata'").execute(&mut *c).await?;sqlx::query("INSERT INTO usage_sessions(id,player_id,balance_id,device_id,location_id,start_time,end_time,end_reason,duration_minutes,wallet_minutes_at_start,created_at,updated_at) VALUES(?,?,?,?,?,?,?,'voluntary',50,120,?,?)").bind(id.to_string()).bind(player.to_string()).bind(balance.to_string()).bind(device.to_string()).bind(venue.to_string()).bind(start).bind(end).bind(start).bind(end).execute(&mut *c).await?;write_outbox_event_on_connection(c,NewOutboxEvent{aggregate_type:"session".into(),aggregate_id:id,event_type:"session.ended".into(),location_id:Some(venue),schema_version:1,deleted:false,payload:json!({"id":id})}).await?;Ok(())})).await.unwrap();
    a.write(|tx| {
        tx.execute_batch("UPDATE _ingest_state SET timezone='Asia/Kolkata'")
            .map_err(error)?;
        Ok(())
    })
    .await
    .unwrap();
    let batch = events(&s.tenant).await;
    apply_batch(a.clone(), batch.clone()).await.unwrap();
    let totals:(i64,i64,i64)=a.read(|tx|tx.query_row("SELECT COUNT(*),SUM(occupied_seconds),COUNT(*) FILTER(WHERE is_start_hour) FROM session_hours",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(error)).await.unwrap();
    assert_eq!(totals, (2, 3000, 1));
    let mut changed = batch.last().unwrap().clone();
    changed.sequence += 1;
    changed.event_id = Uuid::now_v7().to_string();
    let row = &mut changed.analytics_snapshot.as_mut().unwrap().changes[0].rows[0];
    row["end_time"] = json!("2026-10-01T18:25:00.000000Z");
    row["duration_minutes"] = json!(5);
    apply_batch(a.clone(), vec![changed]).await.unwrap();
    assert_eq!(
        a.read(|tx| tx
            .query_row(
                "SELECT COUNT(*),SUM(occupied_seconds) FROM session_hours",
                [],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
            )
            .map_err(error))
            .await
            .unwrap(),
        (1, 300)
    );
    drop(a);
    s.tenant.close().await;
}
#[tokio::test]
async fn fenced_consumer_cannot_change_checkpoint() {
    let f = TenantFixture::new().await;
    f.player("fenced-player").await;
    let a = ready(&f).await;
    let batch = events(&f).await;
    f.revoke();
    assert!(apply_batch(a.clone(), batch).await.is_err());
    drop(a);
    let c = duckdb::Connection::open(f.db.path().with_file_name("analytics.duckdb")).unwrap();
    assert_eq!(
        c.query_row("SELECT last_sequence FROM _ingest_state", [], |r| r
            .get::<_, u64>(0))
            .unwrap(),
        0
    );
    drop(c);
    f.close().await;
}
#[tokio::test]
async fn local_consumer_resumes_checkpoint_and_rebuilds_retention_gaps() {
    use gaming_cafe_api::metrics::Metrics;
    let f = TenantFixture::new().await;
    f.player("local-player").await;
    let a = ready(&f).await;
    let metrics = Metrics::default();
    let mut consumer = LocalConsumer::connect(f.db.clone(), a.clone()).await.unwrap();
    assert!(matches!(consumer.poll(&metrics).await.unwrap(), BatchOutcome::Applied { events: 1, .. }));
    drop(consumer);
    let mut consumer = LocalConsumer::connect(f.db.clone(), a.clone()).await.unwrap();
    assert!(matches!(consumer.poll(&metrics).await.unwrap(), BatchOutcome::Applied { events: 0, .. }));
    f.player("missing-player").await;
    f.player("later-player").await;
    f.db.with_immediate_writer(|c| Box::pin(async move {
        sqlx::query("DELETE FROM outbox_events WHERE sequence=2").execute(c).await?;
        Ok(())
    })).await.unwrap();
    assert_eq!(consumer.poll(&metrics).await.unwrap(), BatchOutcome::RebuildRequired);
    assert_eq!(checkpoint(&a).await, (1, "REBUILDING".into()));
    f.revoke();
    assert!(consumer.poll(&metrics).await.is_err());
    drop(consumer);
    drop(a);
    f.close().await;
}

#[tokio::test]
async fn parent_lines_refunds_and_composite_stock_state_are_exact() {
    let s = SessionFixture::new().await;
    let a = ready(&s.tenant).await;
    let sale = Uuid::now_v7();
    let product = Uuid::now_v7();
    let line = Uuid::now_v7();
    let stock = Uuid::now_v7();
    let first = Uuid::now_v7();
    let second = Uuid::now_v7();
    let venue = s.venue;
    let player = s.player;
    s.tenant.db.with_immediate_writer(move|c|Box::pin(async move{
  let at="2026-10-01T00:00:00.000000Z";
  sqlx::query("INSERT INTO products(id,name,day_price,night_price,created_at,updated_at) VALUES(?,'Exact snack',123456,123456,?,?)").bind(product.to_string()).bind(at).bind(at).execute(&mut *c).await?;
  sqlx::query("INSERT INTO transactions(id,player_id,location_id,transaction_type,amount,paid_amount,cash_amount,payment_method,payment_status,transaction_date,created_at,updated_at) VALUES(?,?,?,'product_purchase',370368,370368,370368,'cash','completed',?,?,?)").bind(sale.to_string()).bind(player.to_string()).bind(venue.to_string()).bind(at).bind(at).bind(at).execute(&mut *c).await?;
  sqlx::query("INSERT INTO transaction_products(id,transaction_id,product_id,product_name,quantity,unit_price,created_at,updated_at) VALUES(?,?,?,'Exact snack',3,123456,?,?)").bind(line.to_string()).bind(sale.to_string()).bind(product.to_string()).bind(at).bind(at).execute(&mut *c).await?;
  write_outbox_event_on_connection(c,NewOutboxEvent{aggregate_type:"transaction".into(),aggregate_id:sale,event_type:"transaction.created".into(),location_id:Some(venue),schema_version:1,deleted:false,payload:json!({"id":sale})}).await?;
  sqlx::query("INSERT INTO inventory_locations(id,venue_location_id,name,kind,created_at,updated_at) VALUES(?,?,'Test store','store',?,?)").bind(stock.to_string()).bind(venue.to_string()).bind(at).bind(at).execute(&mut *c).await?;
  sqlx::query("INSERT INTO location_stock(inventory_location_id,product_id,quantity_pieces,created_at,updated_at) VALUES(?,?,8,?,?)").bind(stock.to_string()).bind(product.to_string()).bind(at).bind(at).execute(&mut *c).await?;
  for (id,delta,quantity) in [(first,8,8),(second,-3,5)] {
   sqlx::query("UPDATE location_stock SET quantity_pieces=? WHERE inventory_location_id=? AND product_id=?").bind(quantity).bind(stock.to_string()).bind(product.to_string()).execute(&mut *c).await?;
   sqlx::query("INSERT INTO stock_movements(id,inventory_location_id,product_id,delta,movement_type,created_at) VALUES(?,?,? ,?,'adjustment',?)").bind(id.to_string()).bind(stock.to_string()).bind(product.to_string()).bind(delta).bind(at).execute(&mut *c).await?;
   write_outbox_event_on_connection(c,NewOutboxEvent{aggregate_type:"stock_movement".into(),aggregate_id:id,event_type:"inventory.stock_changed".into(),location_id:Some(venue),schema_version:1,deleted:false,payload:json!({"id":id})}).await?;
  }
  Ok(())
 })).await.unwrap();
    apply_batch(a.clone(), events(&s.tenant).await)
        .await
        .unwrap();
    let values: (String, String, bool, i64, i64) = a
        .read(|tx| {
            Ok((
                tx.query_row(
                    "SELECT CAST(amount AS VARCHAR) FROM transactions",
                    [],
                    |r| r.get(0),
                )
                .map_err(error)?,
                tx.query_row(
                    "SELECT CAST(line_total AS VARCHAR) FROM transaction_lines",
                    [],
                    |r| r.get(0),
                )
                .map_err(error)?,
                tx.query_row("SELECT is_booked FROM transactions", [], |r| r.get(0))
                    .map_err(error)?,
                tx.query_row("SELECT quantity_pieces FROM location_stock", [], |r| {
                    r.get(0)
                })
                .map_err(error)?,
                tx.query_row("SELECT SUM(delta) FROM stock_movements", [], |r| r.get(0))
                    .map_err(error)?,
            ))
        })
        .await
        .unwrap();
    assert_eq!(values, ("37.0368".into(), "37.0368".into(), true, 5, 5));
    s.tenant
        .db
        .with_immediate_writer(move |c| {
            Box::pin(async move {
                sqlx::query("UPDATE transactions SET payment_status='refunded' WHERE id=?")
                    .bind(sale.to_string())
                    .execute(&mut *c)
                    .await?;
                sqlx::query("DELETE FROM transaction_products WHERE transaction_id=?")
                    .bind(sale.to_string())
                    .execute(&mut *c)
                    .await?;
                write_outbox_event_on_connection(
                    c,
                    NewOutboxEvent {
                        aggregate_type: "transaction".into(),
                        aggregate_id: sale,
                        event_type: "transaction.updated".into(),
                        location_id: Some(venue),
                        schema_version: 1,
                        deleted: false,
                        payload: json!({"id":sale}),
                    },
                )
                .await?;
                Ok(())
            })
        })
        .await
        .unwrap();
    apply_batch(a.clone(), events(&s.tenant).await)
        .await
        .unwrap();
    assert!(!a
        .read(|tx| tx
            .query_row("SELECT is_booked FROM transactions", [], |r| r
                .get::<_, bool>(0))
            .map_err(error))
        .await
        .unwrap());
    assert_eq!(
        a.read(|tx| tx
            .query_row("SELECT COUNT(*) FROM transaction_lines", [], |r| r
                .get::<_, i64>(0))
            .map_err(error))
            .await
            .unwrap(),
        0
    );
    drop(a);
    s.tenant.close().await;
}

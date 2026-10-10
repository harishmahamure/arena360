#![cfg(not(feature = "duckdb-analytics"))]
mod support;
use gaming_cafe_api::{
    analytics::{business::Window, report_reader::ReportReader},
    tenancy::{write_outbox_event_on_connection, NewOutboxEvent},
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;
async fn sale(db: Arc<gaming_cafe_api::tenancy::TenantDb>, player: Uuid, venue: Uuid, amount: i64) {
    db.with_immediate_writer(move |c| Box::pin(async move {
        let id=Uuid::now_v7();
        let at="2026-10-01T12:00:00.000000Z";
        sqlx::query("INSERT INTO transactions(id,player_id,location_id,transaction_type,amount,paid_amount,cash_amount,payment_method,payment_status,transaction_date,created_at,updated_at) VALUES(?,?,?,'product_purchase',?,?,?,'cash','completed',?,?,?)")
            .bind(id.to_string()).bind(player.to_string()).bind(venue.to_string()).bind(amount).bind(amount).bind(amount).bind(at).bind(at).bind(at).execute(&mut *c).await?;
        write_outbox_event_on_connection(c,NewOutboxEvent {location_id:Some(venue),aggregate_type:"transaction".into(),aggregate_id:id,event_type:"transaction.created".into(),schema_version:1,deleted:false,payload:json!({})}).await?;
        Ok(())
    })).await.unwrap();
}
#[tokio::test]
async fn one_report_snapshot_survives_writes_and_new_readers_see_exact_totals() {
    let f = support::TenantFixture::new().await;
    let player = f.player("ledger-player").await;
    let venue = f.venue("main").await;
    sale(f.db.clone(), player, venue, 9_007_199_254_740_993).await;
    let old = ReportReader::new(f.db.clone()).await.unwrap();
    let sql = "SELECT report_money_text(SUM(amount),4) FROM report_transactions";
    let old_key = old.cache_key();
    assert_eq!(
        old.query::<(String,)>(sql).fetch_one().await.unwrap().0,
        "900719925474.0993"
    );
    sale(f.db.clone(), player, venue, 1).await;
    assert_eq!(
        old.query::<(String,)>(sql).fetch_one().await.unwrap().0,
        "900719925474.0993"
    );
    let fresh = ReportReader::new(f.db.clone()).await.unwrap();
    assert_ne!(old_key, fresh.cache_key());
    assert_eq!(
        fresh.query::<(String,)>(sql).fetch_one().await.unwrap().0,
        "900719925474.0994"
    );
    drop((old, fresh));
    f.close().await;
}
#[tokio::test]
async fn venue_scope_empty_grants_and_ownership_are_enforced() {
    let f = support::TenantFixture::new().await;
    let other = support::TenantFixture::new().await;
    let player = f.player("scoped").await;
    let a = f.venue("a").await;
    let b = f.venue("b").await;
    sale(f.db.clone(), player, a, 100_001).await;
    sale(f.db.clone(), player, b, 200_002).await;
    for (venues, amount, count) in [
        (Some(vec![a]), "10.0001", 1),
        (Some(vec![b]), "20.0002", 1),
        (Some(vec![]), "0.0000", 0),
        (None, "30.0003", 2),
    ] {
        let reader = ReportReader::new(f.db.clone())
            .await
            .unwrap()
            .scoped(venues);
        let row:(String,i64)=reader.query("SELECT report_money_text(COALESCE(SUM(amount),0),4),COUNT(*) FROM report_transactions").fetch_one().await.unwrap();
        assert_eq!(row, (amount.to_owned(), count));
        let stats = gaming_cafe_api::services::StatsService::new(
            reader.clone(),
            Arc::new(gaming_cafe_api::cache::NoopCache),
        );
        assert_eq!(
            stats
                .get_dashboard_stats(Some("2026-10-01".into()), Some("2026-10-01".into()), true)
                .await
                .unwrap()
                .transactions
                .current
                .total_transactions,
            count
        );
    }
    let reader = ReportReader::new(other.db.clone()).await.unwrap();
    assert_eq!(
        reader
            .query::<(i64,)>("SELECT COUNT(*) FROM report_transactions")
            .fetch_one()
            .await
            .unwrap()
            .0,
        0
    );
    drop(reader);
    let reader = ReportReader::new(f.db.clone()).await.unwrap();
    f.revoke();
    assert!(reader
        .query::<(i64,)>("SELECT COUNT(*) FROM report_transactions")
        .fetch_one()
        .await
        .is_err());
    drop(reader);
    f.close().await;
    other.close().await;
}
#[tokio::test]
async fn unused_station_has_zero_occupancy_and_closed_sessions_split_at_midnight() {
    let f = support::SessionFixture::new().await;
    let idle = f.second_device().await;
    let session = Uuid::now_v7();
    let (player, balance, device, venue) = (f.player, f.balance, f.device, f.venue);
    f.tenant.db.with_immediate_writer(move |c| Box::pin(async move {
        sqlx::query("UPDATE tenant_runtime SET timezone='Asia/Kolkata'").execute(&mut *c).await?;
        sqlx::query("INSERT INTO usage_sessions(id,player_id,balance_id,device_id,location_id,start_time,end_time,duration_minutes,wallet_minutes_at_start,end_reason,created_at,updated_at) VALUES(?,?,?,?,?,'2026-10-01T18:20:00.000000Z','2026-10-01T19:10:00.000000Z',50,120,'voluntary','2026-10-01T18:20:00.000000Z','2026-10-01T19:10:00.000000Z')")
            .bind(session.to_string()).bind(player.to_string()).bind(balance.to_string()).bind(device.to_string()).bind(venue.to_string()).execute(c).await?;Ok(())
    })).await.unwrap();
    let now = "2026-10-03T00:00:00Z".parse().unwrap();
    let reader = ReportReader::new(f.tenant.db.clone())
        .await
        .unwrap()
        .at(now);
    let report = reader
        .business_report(
            Window::new(
                Some("2026-10-01"),
                Some("2026-10-02"),
                now,
                reader.timezone(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        report
            .stations
            .iter()
            .find(|s| s.id == idle.to_string())
            .unwrap()
            .hours,
        0.0
    );
    let hours: Vec<_> = report
        .hourly_usage
        .iter()
        .map(|r| (r.date.as_str(), r.hour, r.starts, r.hours))
        .collect();
    assert_eq!(
        hours,
        vec![
            ("2026-10-01", 23, 1, 1.0 / 6.0),
            ("2026-10-02", 0, 0, 2.0 / 3.0)
        ]
    );
    drop(reader);
    f.close().await;
}
#[tokio::test]
async fn reporting_views_exclude_credentials_and_time_ranges_use_indexes() {
    use sqlx::Row;
    let f = support::TenantFixture::new().await;
    let pool = f.db.read_pool().unwrap();
    let cols = sqlx::query("PRAGMA table_info(report_base_users)")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(!cols.iter().any(|r| matches!(
        r.get::<String, _>("name").as_str(),
        "password_hash" | "totp_secret" | "permissions"
    )));
    for (sql,index) in [
        ("SELECT SUM(amount) FROM report_base_transactions WHERE created_at>=? AND created_at<?","transactions_created_live"),
        ("SELECT SUM(amount) FROM report_base_transactions WHERE occurred_at>=? AND occurred_at<?","transactions_time_live"),
        ("SELECT SUM(amount) FROM report_base_credit_settlements WHERE settled_at>=? AND settled_at<?","credit_settlements_time_live"),
        ("SELECT SUM(amount) FROM report_base_cash_deposits WHERE created_at>=? AND created_at<?","cash_deposits_created"),
        ("SELECT SUM(variance) FROM report_base_cash_registers WHERE variance IS NOT NULL AND status IN ('closed','reconciled') AND updated_at>=? AND updated_at<?","cash_registers_variance_time"),
    ] {
        let plan=sqlx::query(&format!("EXPLAIN QUERY PLAN {sql}")).bind("2026-10-01T00:00:00.000000Z").bind("2026-10-02T00:00:00.000000Z").fetch_all(&pool).await.unwrap();
        let text=plan.iter().map(|r|r.get::<String,_>("detail")).collect::<Vec<_>>().join("; ");
        assert!(text.contains(index) && text.contains("SEARCH"),"{text}");
    }
    f.close().await;
}
#[tokio::test]
async fn local_retention_never_deletes_unprojected_events_or_resets_sequence() {
    let f = support::TenantFixture::new().await;
    let player = f.player("events").await;
    let venue = f.venue("events").await;
    sale(f.db.clone(), player, venue, 10_000).await;
    let pool = f.db.read_pool().unwrap();
    let max: i64 = sqlx::query_scalar("SELECT MAX(sequence) FROM outbox_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE outbox_events SET occurred_at='2025-01-01T00:00:00.000000Z'")
                .execute(&mut *c)
                .await?;
            sqlx::query("UPDATE realtime_projection_cursor SET sequence=? WHERE singleton=1")
                .bind(max - 1)
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let n =
        gaming_cafe_api::analytics::outbox_retention::prune_batch(f.db.clone(), chrono::Utc::now())
            .await
            .unwrap();
    assert!(n > 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT seq FROM sqlite_sequence WHERE name='outbox_events'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        max
    );
    f.close().await;
}

#[tokio::test]
async fn older_schema_returns_explicit_unavailability_until_rollout() {
    let f = support::TenantFixture::new().await;
    f.db.with_immediate_writer(|c| {
        Box::pin(async move {
            sqlx::query("UPDATE _sqlx_migrations SET success=0 WHERE version=18")
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let error = ReportReader::new(f.db.clone()).await.err().unwrap();
    assert!(error.to_string().contains("ANALYTICS_UNAVAILABLE"));
    f.close().await;
}

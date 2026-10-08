#![cfg(feature = "duckdb-analytics")]
mod support;
use gaming_cafe_api::{
    analytics::{
        report_reader::ReportReader,
        tenant_db::{error, TenantAnalytics},
    },
    cache::NoopCache,
    services::StatsService,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;
#[tokio::test]
async fn location_reports_separate_tenants_locations_and_comparisons() {
    let f = support::TenantFixture::new().await;
    let other = support::TenantFixture::new().await;
    let a = TenantAnalytics::open(f.db.clone()).await.unwrap();
    let b = TenantAnalytics::open(other.db.clone()).await.unwrap();
    for db in [&a, &b] {
        db.write(|tx| {
            tx.execute_batch(
                "UPDATE _ingest_state SET status='READY',hot_window_start=DATE '2025-01-01'",
            )
            .map_err(error)?;
            Ok(())
        })
        .await
        .unwrap();
    }
    let venue_a = Uuid::from_u128(10);
    let venue_b = Uuid::from_u128(11);
    for (db, venue, amount) in [
        (&a, venue_a, "10.00"),
        (&a, venue_b, "20.00"),
        (&b, venue_a, "99.00"),
    ] {
        support::reports::insert(db,"transactions",json!({"id":Uuid::new_v4(),"locationId":venue,"playerId":Uuid::new_v4(),"amount":amount,"paidAmount":amount,"paymentMethod":"cash","paymentStatus":"completed","transactionType":"product_purchase","transactionDate":"2026-10-01T12:00:00Z","createdAt":"2026-10-01T12:00:00Z"})).await;
    }
    for (venues, expected, count) in [
        (Some(vec![venue_a]), "10.00", 1),
        (Some(vec![venue_b]), "20.00", 1),
        (Some(vec![venue_a, venue_b]), "30.00", 2),
        (None, "30.00", 2),
        (Some(vec![]), "0.00", 0),
    ] {
        let reader = ReportReader::new(a.clone()).await.unwrap().scoped(venues);
        let report = reader
            .finance_report(
                "2026-10-01T00:00:00Z".parse().unwrap(),
                "2026-10-02T00:00:00Z".parse().unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(report["sales"], expected);
        assert_eq!(report["saleCount"], count);
        let stats = StatsService::new(reader, Arc::new(NoopCache));
        let dashboard = stats
            .get_dashboard_stats(Some("2026-10-01".into()), Some("2026-10-01".into()), true)
            .await
            .unwrap();
        assert_eq!(dashboard.transactions.current.total_transactions, count);
        assert_eq!(
            dashboard.transactions.previous.unwrap().total_transactions,
            0
        );
        stats
            .get_staff_dashboard_stats(Some("2026-10-01".into()), Some("2026-10-01".into()), None)
            .await
            .unwrap();
        stats
            .get_finance_reconciliation_stats(
                Some("2026-10-01".into()),
                Some("2026-10-01".into()),
                true,
            )
            .await
            .unwrap();
        stats
            .get_finance_deposit_stats(Some("2026-10-01".into()), Some("2026-10-01".into()), true)
            .await
            .unwrap();
        stats
            .get_finance_variance_stats(Some("2026-10-01".into()), Some("2026-10-01".into()), true)
            .await
            .unwrap();
    }
    let foreign = ReportReader::new(b.clone())
        .await
        .unwrap()
        .finance_report(
            "2026-10-01T00:00:00Z".parse().unwrap(),
            "2026-10-02T00:00:00Z".parse().unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign["sales"], "99.00");
    drop((a, b));
    f.close().await;
    other.close().await;
}

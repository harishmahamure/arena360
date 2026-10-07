use gaming_cafe_api::{
    analytics::{
        scope::ReportScope,
        worker::{project, Change},
        ClickHouse,
    },
    cache::NoopCache,
    services::StatsService,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires local ClickHouse; creates and removes a separate test database"]
async fn location_reports_separate_tenants_locations_and_comparisons() {
    gaming_cafe_api::config::load_dotenv();
    let _ = tracing_subscriber::fmt()
        .with_env_filter("error")
        .try_init();
    let base = ClickHouse::from_env();
    let database = format!("scope_test_{}", Uuid::new_v4().simple());
    base.execute(&format!("CREATE DATABASE {database}"), &[])
        .await
        .unwrap();
    let ch = base.clone().with_database(database.clone());
    ch.initialize().await.unwrap();
    ch.execute(
        "INSERT INTO analytics_ready VALUES (1,now64(6)),(2,now64(6))",
        &[],
    )
    .await
    .unwrap();
    let org = Uuid::new_v4();
    let other = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    for (tenant, location, amount) in [(org, a, "10.00"), (org, b, "20.00"), (other, a, "99.00")] {
        let id = Uuid::new_v4();
        let player = Uuid::new_v4();
        let row=project(&Change { schema_version:1,source_table:"transactions".into(),row_id:id,version:1,deleted:false,
            row_data:json!({"id":id,"organizationId":tenant,"venueLocationId":location,"playerId":player,"amount":amount,"paidAmount":amount,"paymentMethod":"cash","paymentStatus":"completed","transactionType":"product_purchase","transactionDate":"2026-10-01T12:00:00Z","createdAt":"2026-10-01T12:00:00Z"}) }).unwrap();
        ch.execute(
            &format!("INSERT INTO transactions_versions FORMAT JSONEachRow\n{row}"),
            &[],
        )
        .await
        .unwrap();
    }
    for (locations, expected, count) in [
        (Some(vec![a]), "10.00", 1),
        (Some(vec![b]), "20.00", 1),
        (Some(vec![a, b]), "30.00", 2),
        (None, "30.00", 2),
        (Some(vec![]), "0.00", 0),
    ] {
        let scoped = ch.clone().scoped(ReportScope {
            organization_id: org,
            locations,
        });
        let report = scoped
            .finance_report(
                "2026-10-01T00:00:00Z".parse().unwrap(),
                "2026-10-02T00:00:00Z".parse().unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(report["sales"], expected);
        assert_eq!(report["saleCount"], count);
        let stats = StatsService::new(scoped.clone(), Arc::new(NoopCache));
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
            .get_finance_reconciliation_stats(None, None, true)
            .await
            .unwrap();
        stats
            .get_finance_deposit_stats(None, None, true)
            .await
            .unwrap();
        stats
            .get_finance_variance_stats(None, None, true)
            .await
            .unwrap();
        scoped
            .business_report(
                gaming_cafe_api::analytics::business::Window::new(
                    Some("2026-10-01"),
                    Some("2026-10-01"),
                    chrono::Utc::now(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
    }
    base.execute(&format!("DROP DATABASE {database}"), &[])
        .await
        .unwrap();
}

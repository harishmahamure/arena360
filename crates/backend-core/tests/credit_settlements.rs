//! Settlement service reads use isolated tenant SQLite; report projections use tenant DuckDB.
mod support;
use gaming_cafe_api::{models::CreditSettlementFilterDto, services::CreditService};
use support::TenantFixture;
use uuid::Uuid;
#[tokio::test]
async fn list_settlements_returns_paginated_result() {
    let f = TenantFixture::new().await;
    let result = CreditService::new()
        .list_settlements_tenant(
            f.db.clone(),
            CreditSettlementFilterDto {
                page: Some(1),
                limit: Some(10),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(result.page, 1);
    assert_eq!(result.limit, 10);
    assert_eq!(result.total, 0);
    assert!(result.data.is_empty());
    f.close().await;
}
#[tokio::test]
async fn get_settlement_returns_not_found_for_missing_id() {
    let f = TenantFixture::new().await;
    let error = CreditService::new()
        .get_settlement_tenant(f.db.clone(), Uuid::now_v7())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        gaming_cafe_api::error::AppError::NotFound(_)
    ));
    f.close().await;
}

#[tokio::test]
#[cfg(feature = "duckdb-analytics")]
async fn revenue_stats_includes_settlement_collections_for_period() {
    let f=TenantFixture::new().await;
    let analytics=gaming_cafe_api::analytics::tenant_db::TenantAnalytics::open(f.db.clone()).await.unwrap();
    analytics.write(|tx| {tx.execute_batch("UPDATE _ingest_state SET status='READY'").map_err(gaming_cafe_api::analytics::tenant_db::error)?;Ok(())}).await.unwrap();
    let reader=gaming_cafe_api::analytics::report_reader::ReportReader::new(analytics.clone()).await.unwrap();
    let stats = gaming_cafe_api::services::StatsService::new(reader,std::sync::Arc::new(gaming_cafe_api::cache::NoopCache));

    let now = chrono::Utc::now();
    let start = now - chrono::Duration::days(30);
    let end = now + chrono::Duration::hours(1);
    let diff = (end - start).num_days().max(1);
    let prev_start = start - chrono::Duration::days(diff);
    let prev_end = end - chrono::Duration::days(diff);

    let revenue = stats
        .get_revenue_by_payment_method(start, end, prev_start, prev_end, false)
        .await
        .expect("revenue stats");

    let current = &revenue.current;
    assert!(current.cash_revenue >= 0.0);
    assert!(current.online_revenue >= 0.0);
    assert!(current.total >= 0.0);

    let plan_from_breakdown =
        current.plan_cash_revenue + current.plan_online_revenue + current.plan_credit_revenue;
    let merchandise_from_breakdown = current.product_cash_revenue
        + current.product_online_revenue
        + current.product_credit_revenue;

    assert!(
        (current.plan - plan_from_breakdown).abs() < 0.01,
        "plan headline {} must equal breakdown sum {}",
        current.plan,
        plan_from_breakdown
    );
    assert!(
        (current.merchandise - merchandise_from_breakdown).abs() < 0.01,
        "merchandise headline {} must equal breakdown sum {}",
        current.merchandise,
        merchandise_from_breakdown
    );
    assert!(
        (current.total - (current.plan + current.merchandise)).abs() < 0.01,
        "total {} must equal plan + merchandise {}",
        current.total,
        current.plan + current.merchandise
    );
    drop((stats,analytics));f.close().await;
}

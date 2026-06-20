//! Integration tests for credit settlement list/detail APIs.
//! Run with: `cargo test --test credit_settlements -- --ignored`

use gaming_cafe_api::app::build_state;
use gaming_cafe_api::config::load_dotenv;
use gaming_cafe_api::models::CreditSettlementFilterDto;
use std::sync::Arc;
use uuid::Uuid;

async fn setup() -> Option<Arc<gaming_cafe_api::app::AppState>> {
    load_dotenv();
    if std::env::var("DATABASE_URL").is_err() && std::env::var("DB_HOST").is_err() {
        return None;
    }
    Some(build_state().await)
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn list_settlements_returns_paginated_result() {
    let Some(state) = setup().await else {
        return;
    };

    let result = state
        .credit
        .list_settlements(CreditSettlementFilterDto {
            page: Some(1),
            limit: Some(10),
            ..Default::default()
        })
        .await
        .expect("list settlements");

    assert!(result.page >= 1);
    assert!(result.limit >= 1);
    assert!(result.total >= 0);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn get_settlement_returns_not_found_for_missing_id() {
    let Some(state) = setup().await else {
        return;
    };

    let err = state
        .credit
        .get_settlement(Uuid::new_v4())
        .await
        .expect_err("missing settlement");

    assert!(err.to_string().contains("not found"));
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn revenue_stats_includes_settlement_collections_for_period() {
    let Some(state) = setup().await else {
        return;
    };

    let now = chrono::Utc::now();
    let start = now - chrono::Duration::days(30);
    let end = now + chrono::Duration::hours(1);

    let revenue = state
        .stats
        .get_revenue_by_payment_method(
            Some(start.to_rfc3339()),
            Some(end.to_rfc3339()),
        )
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
}

#![cfg(feature = "duckdb-analytics")]
mod support;
use gaming_cafe_api::analytics::{
    business::Window,
    report_reader::ReportReader,
    tenant_db::{error, TenantAnalytics},
};
use serde_json::json;
use uuid::Uuid;
#[tokio::test]
async fn business_reports_clip_sessions_and_preserve_metric_grains() {
    let f = support::TenantFixture::new().await;
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
    let analytics = TenantAnalytics::open(f.db.clone()).await.unwrap();
    analytics
        .write(|tx| {
            tx.execute_batch(
                "UPDATE _ingest_state SET status='READY',hot_window_start=DATE '2025-01-01'",
            )
            .map_err(error)?;
            Ok(())
        })
        .await
        .unwrap();
    let ch = ReportReader::new(analytics.clone()).await.unwrap();
    let now = "2026-10-03T00:00:00Z".parse().unwrap();
    let window = || {
        Window::new(
            Some("2026-10-01"),
            Some("2026-10-02"),
            now,
            chrono_tz::Asia::Kolkata,
        )
        .unwrap()
    };
    let empty = ch.business_report(window()).await.unwrap();
    assert_eq!(empty.customers.visitors, 0);
    assert!(empty.daily_sales.is_empty());
    let player = Uuid::new_v4();
    let staff = Uuid::new_v4();
    let device = Uuid::new_v4();
    let plan = Uuid::new_v4();
    let wallet = Uuid::new_v4();
    let product = Uuid::new_v4();
    let sale = Uuid::new_v4();
    for (table, data) in [
        (
            "users",
            json!({"id":player,"username":"player","role":"player","isActive":true,"creditLimit":"0"}),
        ),
        (
            "users",
            json!({"id":staff,"username":"staff","role":"staff","isActive":true,"creditLimit":"0"}),
        ),
        (
            "devices",
            json!({"id":device,"name":"PC 1","status":"available","location":"Main floor"}),
        ),
        (
            "plans",
            json!({"id":plan,"name":"Pass","price":"100","timeCredits":60}),
        ),
        (
            "player_plan_balances",
            json!({"id":wallet,"playerId":player,"status":"active","kind":"time","remainingMinutes":120,"expiryDate":"2099-01-01T00:00:00Z","sourcePlanId":plan}),
        ),
        ("products", json!({"id":product,"name":"Snack"})),
        (
            "usage_sessions",
            json!({"id":Uuid::new_v4(),"balanceId":wallet,"deviceId":device,"startTime":"2026-09-30T10:00:00Z","endTime":"2026-09-30T11:00:00Z","createdBy":staff}),
        ),
        // 23:30 IST until 01:00 IST: contributes half an hour then one hour.
        (
            "usage_sessions",
            json!({"id":Uuid::new_v4(),"balanceId":wallet,"deviceId":device,"startTime":"2026-10-01T18:00:00Z","endTime":"2026-10-01T19:30:00Z","createdBy":staff}),
        ),
        (
            "shifts",
            json!({"id":Uuid::new_v4(),"userId":staff,"clockIn":"2026-10-01T17:30:00Z","clockOut":"2026-10-01T19:30:00Z","status":"completed"}),
        ),
        (
            "transactions",
            json!({"id":sale,"playerId":player,"amount":"25.50","paidAmount":"25.50","paymentMethod":"cash","paymentStatus":"completed","transactionType":"product_purchase","transactionDate":"2026-10-01T18:00:00Z","createdBy":staff}),
        ),
        (
            "transaction_products",
            json!({"id":Uuid::new_v4(),"transactionId":sale,"productId":product,"quantity":2,"unitPrice":"12.75"}),
        ),
        (
            "transactions",
            json!({"id":Uuid::new_v4(),"playerId":player,"planId":plan,"amount":"100.00","paidAmount":"0","paymentMethod":"credit","paymentStatus":"credit","transactionType":"plan_purchase","transactionDate":"2026-10-01T17:00:00Z","createdBy":staff}),
        ),
        (
            "transactions",
            json!({"id":Uuid::new_v4(),"playerId":player,"planId":plan,"amount":"100.00","paidAmount":"100","paymentMethod":"cash","paymentStatus":"completed","transactionType":"plan_purchase","transactionDate":"2026-10-02T10:00:00Z","createdBy":staff}),
        ),
        (
            "transactions",
            json!({"id":Uuid::new_v4(),"playerId":player,"amount":"999","paidAmount":"0","paymentMethod":"cash","paymentStatus":"refunded","transactionType":"product_purchase","transactionDate":"2026-10-01T18:00:00Z"}),
        ),
        (
            "credit_settlements",
            json!({"id":Uuid::new_v4(),"playerId":player,"amount":"100","paymentMethod":"cash","settledAt":"2026-10-01T18:00:00Z"}),
        ),
    ] {
        support::reports::insert(&analytics, table, data).await;
    }
    let report = ch.business_report(window()).await.unwrap();
    assert_eq!(
        report.daily_sales.iter().map(|r| r.revenue).sum::<f64>(),
        225.5,
        "no credit collection double count or refunded sales"
    );
    assert_eq!(report.hourly_usage.len(), 2);
    assert_eq!(report.hourly_usage[0].date, "2026-10-01");
    assert_eq!(report.hourly_usage[0].hour, 23);
    assert_eq!(report.hourly_usage[0].hours, 0.5);
    assert_eq!(report.hourly_usage[1].date, "2026-10-02");
    assert_eq!(report.hourly_usage[1].hour, 0);
    assert_eq!(report.hourly_usage[1].hours, 1.0);
    assert_eq!(report.stations[0].hours, 1.5);
    assert_eq!(report.stations[0].sessions, 1);
    assert_eq!(report.customers.visitors, 1);
    assert_eq!(report.customers.repeat_visitors, 1);
    assert_eq!(report.customers.retained_visitors, 1);
    assert_eq!(report.pos.attached_customers, 1);
    assert_eq!(report.pos.revenue, 25.5);
    assert_eq!(report.products[0].revenue, 25.5);
    assert_eq!(report.plans[0].repeat_buyers, 1);
    assert_eq!(report.wallets.holders, 1);
    assert_eq!(report.staff[0].shift_hours, 2.0);
    assert_eq!(report.staff[0].revenue, 225.5);
    drop((ch, analytics));
    f.close().await;
}

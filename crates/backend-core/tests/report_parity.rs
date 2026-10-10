mod support;
use gaming_cafe_api::{
    analytics::{
        business::Window,
        report_reader::ReportReader,
    },
    cache::NoopCache,
    services::StatsService,
};
#[cfg(feature = "duckdb-analytics")]
use gaming_cafe_api::analytics::tenant_db::{error, TenantAnalytics};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};

const FIXTURES: &[(&str, &str)] = &[
    (
        "credit-summary",
        include_str!("fixtures/reports/credit-summary.json"),
    ),
    (
        "expenses-summary",
        include_str!("fixtures/reports/expenses-summary.json"),
    ),
    (
        "inventory-overview",
        include_str!("fixtures/reports/inventory-overview.json"),
    ),
    (
        "inventory-receipts-summary",
        include_str!("fixtures/reports/inventory-receipts-summary.json"),
    ),
    (
        "inventory-waste-summary",
        include_str!("fixtures/reports/inventory-waste-summary.json"),
    ),
    (
        "stats-business.day",
        include_str!("fixtures/reports/stats-business.day.json"),
    ),
    (
        "stats-business.month",
        include_str!("fixtures/reports/stats-business.month.json"),
    ),
    (
        "stats-business.week",
        include_str!("fixtures/reports/stats-business.week.json"),
    ),
    (
        "stats-dashboard.day",
        include_str!("fixtures/reports/stats-dashboard.day.json"),
    ),
    (
        "stats-dashboard.month",
        include_str!("fixtures/reports/stats-dashboard.month.json"),
    ),
    (
        "stats-dashboard.week",
        include_str!("fixtures/reports/stats-dashboard.week.json"),
    ),
    (
        "stats-finance-deposits.day",
        include_str!("fixtures/reports/stats-finance-deposits.day.json"),
    ),
    (
        "stats-finance-deposits.month",
        include_str!("fixtures/reports/stats-finance-deposits.month.json"),
    ),
    (
        "stats-finance-deposits.week",
        include_str!("fixtures/reports/stats-finance-deposits.week.json"),
    ),
    (
        "stats-finance-reconciliation.day",
        include_str!("fixtures/reports/stats-finance-reconciliation.day.json"),
    ),
    (
        "stats-finance-reconciliation.month",
        include_str!("fixtures/reports/stats-finance-reconciliation.month.json"),
    ),
    (
        "stats-finance-reconciliation.week",
        include_str!("fixtures/reports/stats-finance-reconciliation.week.json"),
    ),
    (
        "stats-finance-report.day",
        include_str!("fixtures/reports/stats-finance-report.day.json"),
    ),
    (
        "stats-finance-report.month",
        include_str!("fixtures/reports/stats-finance-report.month.json"),
    ),
    (
        "stats-finance-variance.day",
        include_str!("fixtures/reports/stats-finance-variance.day.json"),
    ),
    (
        "stats-finance-variance.month",
        include_str!("fixtures/reports/stats-finance-variance.month.json"),
    ),
    (
        "stats-finance-variance.week",
        include_str!("fixtures/reports/stats-finance-variance.week.json"),
    ),
    (
        "stats-revenue-by-payment-method.day",
        include_str!("fixtures/reports/stats-revenue-by-payment-method.day.json"),
    ),
    (
        "stats-revenue-by-payment-method.month",
        include_str!("fixtures/reports/stats-revenue-by-payment-method.month.json"),
    ),
    (
        "stats-revenue-by-payment-method.week",
        include_str!("fixtures/reports/stats-revenue-by-payment-method.week.json"),
    ),
    (
        "stats-staff-dashboard.day",
        include_str!("fixtures/reports/stats-staff-dashboard.day.json"),
    ),
    (
        "stats-staff-dashboard.month",
        include_str!("fixtures/reports/stats-staff-dashboard.month.json"),
    ),
    (
        "stats-staff-dashboard.week",
        include_str!("fixtures/reports/stats-staff-dashboard.week.json"),
    ),
    (
        "stats-usage.day",
        include_str!("fixtures/reports/stats-usage.day.json"),
    ),
    (
        "stats-usage.month",
        include_str!("fixtures/reports/stats-usage.month.json"),
    ),
    (
        "stats-usage.week",
        include_str!("fixtures/reports/stats-usage.week.json"),
    ),
];
fn normalize(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("generatedAt");
            for (_, value) in map.iter_mut() {
                normalize(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                normalize(value);
            }
        }
        Value::String(text) => {
            if let Ok(t) = chrono::DateTime::parse_from_rfc3339(text) {
                *text = t
                    .with_timezone(&chrono::Utc)
                    .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
            }
        }
        _ => {}
    }
}

// M0 sorted ties after LIMIT 8. Its final timestamp has five movements for four
// remaining slots, so that subset is unspecified by the old query. Validate all
// captured rows against the immutable inputs, then use the agreed ID tie-breaker
// before LIMIT. The committed golden JSON stays unchanged.
fn normalize_movement_boundary(expected: &mut Value, input: &Value) {
    let mut source = Vec::new();
    for original in input["tables"]["stock_movements"].as_array().unwrap() {
        let mut row = original.clone();
        for key in ["createdBy", "referenceId", "referenceType"] {
            if row.get(key).is_none() {
                row[key] = Value::Null;
            }
        }
        let location = input["tables"]["inventory_locations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == row["locationId"])
            .unwrap();
        let product = input["tables"]["products"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == row["productId"])
            .unwrap();
        row["locationName"] = location["name"].clone();
        row["productName"] = product["name"].clone();
        normalize(&mut row);
        source.push(row);
    }
    source.sort_by(|a, b| {
        b["createdAt"]
            .as_str()
            .unwrap()
            .cmp(a["createdAt"].as_str().unwrap())
            .then(a["id"].as_str().unwrap().cmp(b["id"].as_str().unwrap()))
    });
    let captured = expected["recentMovements"].as_array().unwrap();
    let count = captured.len();
    assert!(count > 0 && source.len() > count);
    let cutoff = &source[count - 1]["createdAt"];
    assert_eq!(
        cutoff, &source[count]["createdAt"],
        "M0 boundary must actually be tied"
    );
    for row in captured {
        let original = source.iter().find(|v| v["id"] == row["id"]).unwrap();
        assert_eq!(row, original, "M0 movement must match its original input");
    }
    let above = source
        .iter()
        .take_while(|r| &r["createdAt"] != cutoff)
        .count();
    assert_eq!(&captured[..above], &source[..above]);
    assert!(captured[above..].iter().all(|r| &r["createdAt"] == cutoff));
    expected["recentMovements"] = Value::Array(source.into_iter().take(count).collect());
}
// The captured M0 SQL counted a full window for an unused station because
// DuckDB GREATEST ignored NULL from its left join. Recompute occupancy directly
// from the immutable session inputs; do not preserve those phantom hours.
fn correct_station_occupancy(expected: &mut Value, input: &Value, window: &Window) {
    let mut stations = expected["stations"].as_array().unwrap().clone();
    for station in &mut stations {
        let id = station["id"].as_str().unwrap();
        let mut seconds = 0i64;
        for session in input["tables"]["usage_sessions"].as_array().unwrap() {
            if session["deviceId"]!=id {continue;}
            let a: chrono::DateTime<chrono::Utc> = session["startTime"].as_str().unwrap().parse().unwrap();
            let b: chrono::DateTime<chrono::Utc> = session["endTime"].as_str().map(|s|s.parse().unwrap()).unwrap_or(window.end);
            seconds += (b.min(window.end).timestamp()-a.max(window.start).timestamp()).max(0);
        }
        station["hours"] = serde_json::json!(seconds as f64/3600.0);
    }
    stations.sort_by(|a,b| b["hours"].as_f64().unwrap().total_cmp(&a["hours"].as_f64().unwrap()).then(a["name"].as_str().unwrap().cmp(b["name"].as_str().unwrap())));
    expected["stations"] = stations.into();
}
fn difference(expected: &Value, actual: &Value, path: &str) -> Option<String> {
    match (expected, actual) {
        (Value::Object(a), Value::Object(b)) => {
            if a.len() != b.len() {
                return Some(format!(
                    "{path}: expected keys {:?}, got {:?}",
                    a.keys(),
                    b.keys()
                ));
            }
            for (key, value) in a {
                if let Some(diff) = difference(
                    value,
                    b.get(key).unwrap_or(&Value::Null),
                    &format!("{path}.{key}"),
                ) {
                    return Some(diff);
                }
            }
            None
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                return Some(format!(
                    "{path}: expected {} rows, got {}",
                    a.len(),
                    b.len()
                ));
            }
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                if let Some(diff) = difference(a, b, &format!("{path}[{i}]")) {
                    return Some(diff);
                }
            }
            None
        }
        (Value::Number(a), Value::Number(b)) => {
            if a.as_i64().zip(b.as_i64()).is_some_and(|(a, b)| a == b) {
                return None;
            }
            if a.as_f64()
                .zip(b.as_f64())
                .is_some_and(|(a, b)| (a - b).abs() < 1e-8)
            {
                return None;
            }
            Some(format!("{path}: expected {a}, got {b}"))
        }
        _ if expected == actual => None,
        _ => Some(format!("{path}: expected {expected}, got {actual}")),
    }
}
#[tokio::test]
async fn all_31_reports_match_the_original_m0_demo() {
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
    #[cfg(feature = "duckdb-analytics")]
    let analytics = TenantAnalytics::open(f.db.clone()).await.unwrap();
    #[cfg(feature = "duckdb-analytics")]
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
    let input: Value =
        serde_json::from_str(include_str!("fixtures/native_report_demo.json")).unwrap();
    // Explicit parent order; fixture credentials and operational-only ledgers are absent.
    for table in [
        "users",
        "devices",
        "plans",
        "products",
        "vendors",
        "inventory_locations",
        "expense_categories",
        "shifts",
        "transactions",
        "player_plan_balances",
        "usage_sessions",
        "transaction_products",
        "credit_settlements",
        "credit_settlement_items",
        "expenses",
        "cash_registers",
        "cash_deposits",
        "stock_receipts",
        "stock_receipt_lines",
        "stock_waste_events",
        "stock_waste_lines",
        "location_stock",
        "inventory_reorder_rules",
        "stock_movements",
    ] {
        for row in input["tables"][table].as_array().unwrap() {
            #[cfg(feature = "duckdb-analytics")]
            support::reports::insert(&analytics, table, row.clone()).await;
            #[cfg(not(feature = "duckdb-analytics"))]
            support::sqlite_reports::insert(&f.db, table, row.clone()).await;
        }
    }
    // Fixture commit c523ea5 was recorded at this instant. The capture's wallet
    // counts are unchanged throughout the surrounding interval.
    let now = "2026-10-05T21:49:38Z".parse().unwrap();
    #[cfg(feature = "duckdb-analytics")]
    let reader = ReportReader::new(analytics.clone()).await.unwrap().at(now);
    #[cfg(not(feature = "duckdb-analytics"))]
    let reader = ReportReader::new(f.db.clone()).await.unwrap().at(now);
    let stats = StatsService::new(reader.clone(), Arc::new(NoopCache));
    let mut failures = Vec::new();
    for (name, text) in FIXTURES {
        let fixture: Value = serde_json::from_str(text).unwrap();
        let request = fixture["request"].as_str().unwrap();
        let url = reqwest::Url::parse(&format!("http://fixture.local{request}")).unwrap();
        let params: HashMap<String, String> = url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        let start = params.get("startDate").cloned();
        let end = params.get("endDate").cloned();
        let result: Result<Value, gaming_cafe_api::error::AppError> = async {
            Ok(match url.path() {
                "/stats/dashboard" => {
                    serde_json::to_value(stats.get_dashboard_stats(start, end, true).await?)
                        .unwrap()
                }
                "/stats/staff-dashboard" => {
                    serde_json::to_value(stats.get_staff_dashboard_stats(start, end, None).await?)
                        .unwrap()
                }
                "/stats/revenue/by-payment-method" | "/stats/usage" => {
                    let (a, b) = stats.resolve_stats_period(start, end);
                    let (c, d) = stats.previous_window(a, b);
                    if url.path().ends_with("usage") {
                        serde_json::to_value(stats.get_usage_stats(a, b, c, d, true).await?)
                            .unwrap()
                    } else {
                        serde_json::to_value(
                            stats
                                .get_revenue_by_payment_method(a, b, c, d, true)
                                .await?,
                        )
                        .unwrap()
                    }
                }
                "/stats/finance/reconciliation" => serde_json::to_value(
                    stats
                        .get_finance_reconciliation_stats(start, end, true)
                        .await?,
                )
                .unwrap(),
                "/stats/finance/deposits" => {
                    serde_json::to_value(stats.get_finance_deposit_stats(start, end, true).await?)
                        .unwrap()
                }
                "/stats/finance/variance" => {
                    serde_json::to_value(stats.get_finance_variance_stats(start, end, true).await?)
                        .unwrap()
                }
                "/stats/business" => serde_json::to_value(
                    stats
                        .get_business_report(Window::new(
                            start.as_deref(),
                            end.as_deref(),
                            now,
                            reader.timezone(),
                        )?)
                        .await?,
                )
                .unwrap(),
                "/stats/finance/report" => {
                    let start = start.unwrap();
                    let end = end.unwrap();
                    let a = chrono::NaiveDate::parse_from_str(&start, "%Y-%m-%d").unwrap();
                    let b = chrono::NaiveDate::parse_from_str(&end, "%Y-%m-%d")
                        .unwrap()
                        .succ_opt()
                        .unwrap();
                    let mut report = reader
                        .finance_report(
                            a.and_hms_opt(0, 0, 0).unwrap().and_utc(),
                            b.and_hms_opt(0, 0, 0).unwrap().and_utc(),
                        )
                        .await?;
                    report["startDate"] = start.into();
                    report["endDate"] = end.into();
                    report["timezone"] = "UTC".into();
                    report["currency"] = "INR".into();
                    report["locationLabel"] = "All locations".into();
                    report
                }
                "/expenses/summary" => {
                    serde_json::to_value(reader.get_summary_by_category().await?).unwrap()
                }
                "/credit/summary" => {
                    serde_json::to_value(reader.get_portfolio_summary().await?).unwrap()
                }
                "/inventory/overview" => serde_json::to_value(reader.overview().await?).unwrap(),
                "/inventory/receipts/summary" => serde_json::to_value(
                    reader
                        .receipt_summary(
                            None,
                            params.get("from").map(|v| v.parse().unwrap()),
                            params.get("to").map(|v| v.parse().unwrap()),
                        )
                        .await?,
                )
                .unwrap(),
                "/inventory/waste/summary" => serde_json::to_value(
                    reader
                        .waste_summary(
                            None,
                            params.get("from").map(|v| v.parse().unwrap()),
                            params.get("to").map(|v| v.parse().unwrap()),
                        )
                        .await?,
                )
                .unwrap(),
                other => panic!("Uncovered fixture route {other}"),
            })
        }
        .await;
        match result {
            Ok(mut actual) => {
                let mut expected = fixture["data"].clone();
                normalize(&mut actual);
                normalize(&mut expected);
                if name.starts_with("stats-business.") {
                    correct_station_occupancy(&mut expected, &input, &Window::new(
                        params.get("startDate").map(String::as_str), params.get("endDate").map(String::as_str), now, reader.timezone()).unwrap());
                }
                if *name == "inventory-overview" {
                    normalize_movement_boundary(&mut expected, &input);
                }
                if let Some(diff) = difference(&expected, &actual, "$") {
                    failures.push(format!("{name}: {diff}"));
                }
            }
            Err(error) => failures.push(format!("{name}: {error}")),
        }
    }
    drop((stats, reader));
    #[cfg(feature = "duckdb-analytics")]
    drop(analytics);
    f.close().await;
    assert!(
        failures.is_empty(),
        "Report differences:\n{}",
        failures.join("\n")
    );
}

//! Load the original captured report inputs into the real STRICT operational schema.
use gaming_cafe_api::{tenancy::TenantDb, time::format_sqlite_timestamp};
use serde_json::Value;
use sqlx::Row;
use std::sync::Arc;
const VENUE: &str = "11111111-1111-4111-8111-111111111111";
fn camel(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in name.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.push(c.to_ascii_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}
pub async fn insert(db: &Arc<TenantDb>, table: &str, mut input: Value) {
    let info = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(&db.read_pool().unwrap())
        .await
        .unwrap();
    let time = input
        .get("createdAt")
        .cloned()
        .unwrap_or(Value::String("2026-08-04T00:00:00Z".into()));
    input["createdAt"] = time.clone();
    if input.get("updatedAt").is_none() {
        input["updatedAt"] = time.clone();
    }
    if table == "users" && input["role"] == "player" {
        input["passwordHash"] = "fixture-only".into();
    }
    if table == "shifts" && input["status"] == "completed" {
        input["status"] = "closed".into();
    }
    if table == "transaction_products" {
        input["productName"] = "Fixture product".into();
    }
    if table == "usage_sessions" {
        let (player,): (String,) =
            sqlx::query_as("SELECT player_id FROM player_plan_balances WHERE id=?")
                .bind(input["balanceId"].as_str().unwrap())
                .fetch_one(&db.read_pool().unwrap())
                .await
                .unwrap();
        input["playerId"] = player.into();
        if !input["endTime"].is_null() {
            input["endReason"] = "voluntary".into();
        }
    }
    if table == "transactions" {
        if input["paymentStatus"] != "completed" && input["paymentStatus"] != "credit" {
            input["cashAmount"] = 0.into();
            input["onlineAmount"] = 0.into();
        }
        for key in ["cashAmount", "onlineAmount"] {
            if input[key].is_null() {
                input[key] = 0.into();
            }
        }
        if input["paymentStatus"] == "completed" && input["paymentMethod"] != "credit" {
            input["paidAmount"] = input["amount"].clone();
            if input["paymentMethod"] == "cash" {
                input["cashAmount"] = input["amount"].clone();
            }
            if input["paymentMethod"] == "online" {
                input["onlineAmount"] = input["amount"].clone();
            }
        }
    }
    if table == "credit_settlements" {
        if input["paymentMethod"] == "cash" {
            input["cashAmount"] = input["amount"].clone();
            input["onlineAmount"] = 0.into();
        }
        if input["paymentMethod"] == "online" {
            input["onlineAmount"] = input["amount"].clone();
            input["cashAmount"] = 0.into();
        }
    }
    let mut columns = Vec::new();
    let mut values = Vec::new();
    for column in info {
        let name: String = column.get("name");
        let key = camel(&name);
        let mut value = input.get(&key).cloned();
        if name == "inventory_location_id" {
            value = input.get("locationId").cloned();
        }
        if (name == "venue_location_id" || name == "location_id") && value.is_none() {
            value = Some(VENUE.into());
        }
        if name == "receipt_date" && value.is_none() {
            value = Some(time.clone());
        }
        let Some(mut value) = value else { continue };
        if let Some(text) = value.as_str() {
            if let Ok(t) = chrono::DateTime::parse_from_rfc3339(text) {
                value = format_sqlite_timestamp(&t.with_timezone(&chrono::Utc))
                    .unwrap()
                    .into();
            }
        }
        if matches!(
            name.as_str(),
            "amount"
                | "amount_applied"
                | "paid_amount"
                | "cash_amount"
                | "online_amount"
                | "credit_limit"
                | "budget_amount"
                | "price"
                | "day_price"
                | "night_price"
                | "purchase_price"
                | "purchase_price_per_box"
                | "per_minute_rate"
                | "unit_price"
                | "price_at_purchase"
                | "subtotal"
                | "opening_balance"
                | "closing_balance"
                | "expected_closing"
                | "variance"
        ) && !value.is_null()
        {
            let decimal: rust_decimal::Decimal =
                value.to_string().trim_matches('"').parse().unwrap();
            use rust_decimal::prelude::ToPrimitive;
            value = (decimal * rust_decimal::Decimal::from(10_000))
                .to_i64()
                .unwrap()
                .into();
        }
        // Operational schema defaults are deliberate; captured native fixtures omit optional values.
        let required: i64 = column.get("notnull");
        let default: Option<String> = column.get("dflt_value");
        if value.is_null() && required != 0 && default.is_some() {
            continue;
        }
        columns.push(name);
        values.push(value);
    }
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({})",
        columns.join(","),
        vec!["?"; values.len()].join(",")
    );
    let table = table.to_owned();
    db.with_immediate_writer(move |c| Box::pin(async move {
        sqlx::query("INSERT OR IGNORE INTO venue_locations(id,name,slug,created_at,updated_at) VALUES(?,'Fixture venue','fixture','2026-08-04T00:00:00.000000Z','2026-08-04T00:00:00.000000Z')").bind(VENUE).execute(&mut *c).await?;
        let mut query=sqlx::query(&sql);
        for value in values {query=match value {
            Value::Null=>query.bind(None::<String>),Value::String(s)=>query.bind(s),Value::Bool(v)=>query.bind(i64::from(v)),Value::Number(n)=>query.bind(n.as_i64().unwrap()),v=>query.bind(v.to_string())
        };}
        query.execute(c).await.map_err(|e| gaming_cafe_api::error::AppError::Internal(format!("fixture {table}: {e}")))?;
        Ok(())
    })).await.unwrap();
}

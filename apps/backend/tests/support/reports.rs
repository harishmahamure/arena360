//! Original report fixture inputs projected into the native schema for parity tests.
use gaming_cafe_api::{
    analytics::{
        consumer::{derive_labels, insert_row, replace_hours},
        tenant_db::{error, TenantAnalytics},
    },
    tenancy::analytics_snapshot::{ColumnKind, TABLES},
};
use serde_json::{Map, Value};
use std::sync::Arc;

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
pub async fn insert(analytics: &Arc<TenantAnalytics>, table: &str, input: Value) {
    let native = match table {
        "usage_sessions" => "sessions",
        "player_plan_balances" => "wallets",
        "transaction_products" => "transaction_lines",
        "inventory_reorder_rules" => "reorder_rules",
        other => other,
    };
    let spec = TABLES.iter().find(|s| s.name == native).unwrap();
    let mut row = Map::new();
    for column in spec.columns {
        let source = column.source.strip_prefix("r.").unwrap_or(column.name);
        let mut value = input
            .get(camel(source))
            .or_else(|| input.get(camel(column.name)))
            .cloned()
            .unwrap_or(Value::Null);
        if column.kind == ColumnKind::StockKey {
            value = Value::String(
                gaming_cafe_api::tenancy::analytics_snapshot::stock_id(
                    input["locationId"].as_str().unwrap().parse().unwrap(),
                    input["productId"].as_str().unwrap().parse().unwrap(),
                )
                .to_string(),
            );
        }
        if value.is_null() {
            value = match column.name {
                "is_active" => Value::Bool(true),
                "credit_limit" => Value::String("0.0000".into()),
                "is_staff_allowance" => Value::Bool(false),
                _ => value,
            };
        }
        if !value.is_null() {
            match column.kind {
                ColumnKind::Money => {
                    value = Value::String(match value {
                        Value::String(s) => s,
                        other => other.to_string(),
                    });
                }
                ColumnKind::Timestamp => {
                    let t = chrono::DateTime::parse_from_rfc3339(value.as_str().unwrap())
                        .unwrap()
                        .with_timezone(&chrono::Utc);
                    value =
                        Value::String(gaming_cafe_api::time::format_sqlite_timestamp(&t).unwrap());
                }
                _ => {}
            }
        }
        row.insert(column.name.into(), value);
    }
    let native = native.to_owned();
    analytics.write(move|tx| {
        let zone:String=tx.query_row("SELECT timezone FROM _ingest_state",[],|r|r.get(0)).map_err(error)?;
        let zone=zone.parse().unwrap();
        if native=="sessions" {
            let (player,kind,plan):(String,Option<String>,Option<String>)=tx.query_row("SELECT CAST(player_id AS VARCHAR),kind,CAST(source_plan_id AS VARCHAR) FROM wallets WHERE id=CAST(? AS UUID)",duckdb::params![row["balance_id"].as_str().unwrap()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(error)?;
            row.insert("player_id".into(),Value::String(player));
            row.insert("is_staff_allowance".into(),Value::Bool(kind.as_deref()==Some("staff_allowance")));
            row.insert("source_plan_id".into(),plan.map(Value::String).unwrap_or(Value::Null));
        }
        let mut row=Value::Object(row);
        let spec=TABLES.iter().find(|s|s.name==native).unwrap();
        derive_labels(spec,&mut row,zone)?;
        insert_row(tx,spec,&row)?;
        if native=="sessions" {replace_hours(tx,row["id"].as_str().unwrap(),zone)?;}
        Ok(())
    }).await.unwrap();
}

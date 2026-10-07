//! Legacy ClickHouse projection validation retained until M7; no operational database worker.
use super::client::unavailable;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Change {
    pub schema_version: u8,
    pub source_table: String,
    pub row_id: Uuid,
    pub version: u64,
    pub deleted: bool,
    pub row_data: Value,
}

pub fn schema() -> &'static BTreeMap<String, BTreeMap<String, String>> {
    static SCHEMA: std::sync::LazyLock<BTreeMap<String, BTreeMap<String, String>>> =
        std::sync::LazyLock::new(|| {
            serde_json::from_str(include_str!("schema.json")).expect("checked-in analytics schema")
        });
    &SCHEMA
}

pub fn project(change: &Change) -> Result<Value, crate::error::AppError> {
    if change.schema_version != 1 {
        return Err(unavailable("Unsupported analytics schema version"));
    }
    let schema = schema();
    let columns = schema
        .get(&change.source_table)
        .ok_or_else(|| unavailable("Unknown analytics source"))?;
    if change.row_data.get("id").and_then(Value::as_str) != Some(change.row_id.to_string().as_str())
    {
        return Err(unavailable("Analytics row identity mismatch"));
    }
    let mut row = Map::new();
    for (name, kind) in columns {
        let value = change.row_data.get(name).cloned().unwrap_or(Value::Null);
        if value.is_null() && !kind.starts_with("Nullable(") {
            return Err(unavailable(format!(
                "Missing required analytics column: {}.{name}",
                change.source_table
            )));
        }
        row.insert(name.clone(), value);
    }
    row.insert("_version".into(), change.version.into());
    row.insert("_deleted".into(), u8::from(change.deleted).into());
    Ok(Value::Object(row))
}

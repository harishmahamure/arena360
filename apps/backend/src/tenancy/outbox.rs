use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{Sqlite, SqliteConnection, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::time::{format_sqlite_timestamp, parse_sqlite_timestamp, validate_utc_timestamps};

#[derive(Debug, Clone)]
pub struct NewOutboxEvent {
    pub location_id: Option<Uuid>,
    pub aggregate_type: String,
    pub aggregate_id: Uuid,
    pub event_type: String,
    pub schema_version: u32,
    pub deleted: bool,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenOutboxEvent {
    pub sequence: i64,
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

/// Writes an event snapshot through the caller's transaction.
///
/// The business mutation and this insert commit or roll back together.
pub async fn write_outbox_event(
    transaction: &mut Transaction<'_, Sqlite>,
    event: NewOutboxEvent,
) -> Result<WrittenOutboxEvent, AppError> {
    write_outbox_event_on_connection(&mut **transaction, event).await
}

/// Writes an event through a connection that is already inside an explicit transaction.
///
/// Tenant repositories use this with `BEGIN IMMEDIATE`, which SQLx's default transaction
/// constructor does not guarantee.
pub async fn write_outbox_event_on_connection(
    connection: &mut SqliteConnection,
    event: NewOutboxEvent,
) -> Result<WrittenOutboxEvent, AppError> {
    validate_event(&event)?;
    let event_id = Uuid::now_v7();
    let occurred_at_text = format_sqlite_timestamp(&Utc::now())
        .map_err(|error| AppError::Internal(format!("format outbox timestamp: {error}")))?;
    let occurred_at = parse_sqlite_timestamp(&occurred_at_text)
        .map_err(|error| AppError::Internal(format!("parse outbox timestamp: {error}")))?;
    let analytics_snapshot = super::analytics_snapshot::capture(connection, &event.aggregate_type, event.aggregate_id, event.deleted).await?
        .map(|snapshot| serde_json::to_string(&snapshot)).transpose()
        .map_err(|error| AppError::Internal(format!("serialize analytics snapshot: {error}")))?;
    let payload = serde_json::to_string(&event.payload)
        .map_err(|error| AppError::Internal(format!("serialize outbox payload: {error}")))?;
    let sequence: i64 = sqlx::query_scalar(
        r#"INSERT INTO outbox_events
             (event_id, location_id, aggregate_type, aggregate_id, event_type,
              occurred_at, schema_version, deleted, payload, analytics_snapshot)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
           RETURNING sequence"#,
    )
    .bind(event_id.to_string())
    .bind(event.location_id.map(|id| id.to_string()))
    .bind(event.aggregate_type)
    .bind(event.aggregate_id.to_string())
    .bind(event.event_type)
    .bind(occurred_at_text)
    .bind(i64::from(event.schema_version))
    .bind(event.deleted)
    .bind(payload)
    .bind(analytics_snapshot)
    .fetch_one(&mut *connection)
    .await?;

    Ok(WrittenOutboxEvent {
        sequence,
        event_id,
        occurred_at,
    })
}

fn validate_event(event: &NewOutboxEvent) -> Result<(), AppError> {
    validate_name("aggregate type", &event.aggregate_type)?;
    validate_name("event type", &event.event_type)?;
    if event.schema_version == 0 {
        return Err(AppError::Internal(
            "outbox schema version must be greater than zero".into(),
        ));
    }
    if !event.payload.is_object() {
        return Err(AppError::Internal(
            "outbox payload must be a JSON object".into(),
        ));
    }
    validate_utc_timestamps(&event.payload)
        .map_err(|error| AppError::Internal(format!("outbox payload contains {error}")))?;
    validate_no_secrets(&event.payload, "$")
}

fn validate_name(label: &str, value: &str) -> Result<(), AppError> {
    if value.is_empty() || value.trim() != value {
        return Err(AppError::Internal(format!(
            "outbox {label} must be non-empty and trimmed"
        )));
    }
    Ok(())
}

fn validate_no_secrets(value: &Value, path: &str) -> Result<(), AppError> {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                let normalized = key.to_ascii_lowercase().replace(['_', '-'], "");
                if is_sensitive_key(&normalized) {
                    return Err(AppError::Internal(format!(
                        "outbox payload contains forbidden field {path}.{key}"
                    )));
                }
                validate_no_secrets(value, &format!("{path}.{key}"))?;
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_no_secrets(value, &format!("{path}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_sensitive_key(key: &str) -> bool {
    key.contains("password")
        || key.ends_with("token")
        || key.ends_with("secret")
        || key == "otp"
        || key.ends_with("otpcode")
        || matches!(key, "authorization" | "apikey" | "privatekey")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(payload: Value) -> NewOutboxEvent {
        NewOutboxEvent {
            location_id: None,
            aggregate_type: "session".into(),
            aggregate_id: Uuid::new_v4(),
            event_type: "session.updated".into(),
            schema_version: 1,
            deleted: false,
            payload,
        }
    }

    #[test]
    fn rejects_non_objects_local_timestamps_and_secrets() {
        assert!(validate_event(&event(json!([]))).is_err());
        assert!(validate_event(&event(json!({
            "startedAt": "2026-10-06T12:00:00+05:30"
        })))
        .is_err());
        assert!(validate_event(&event(json!({
            "profile": {"passwordHash": "never publish this"}
        })))
        .is_err());
    }
}

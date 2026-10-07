//! Shared fenced write and exact-value boundaries for back-office repositories.
use crate::error::AppError;
use crate::tenancy::{
    decimal_to_scale4, format_sqlite_timestamp, scale4_to_decimal,
    write_outbox_event_on_connection, NewOutboxEvent, TenantDb,
};
use chrono::Utc;
use futures::future::BoxFuture;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use serde_json::Value;
use sqlx::SqliteConnection;
use std::str::FromStr;
use uuid::Uuid;
pub(crate) type WriteOperation<T> = Box<
    dyn for<'connection> FnOnce(
            &'connection mut SqliteConnection,
        ) -> BoxFuture<'connection, Result<T, AppError>>
        + Send,
>;

pub(crate) async fn write<T: Send + 'static>(
    db: &TenantDb,
    operation: WriteOperation<T>,
) -> Result<T, AppError> {
    db.with_immediate_writer(operation).await
}

pub(crate) fn now() -> Result<String, AppError> {
    format_sqlite_timestamp(&Utc::now())
        .map_err(|error| AppError::Internal(format!("format tenant timestamp: {error}")))
}

pub(crate) fn money(value: f64) -> Result<i64, AppError> {
    if !value.is_finite() {
        return Err(AppError::BadRequest("Money must be finite".into()));
    }
    let decimal = Decimal::from_str(&value.to_string())
        .map_err(|_| AppError::BadRequest("Invalid money value".into()))?;
    decimal_to_scale4(decimal)
        .map_err(|error| AppError::BadRequest(format!("Invalid money value: {error}")))
}

pub(crate) fn money_f64(value: i64) -> Result<f64, AppError> {
    scale4_to_decimal(value)
        .to_f64()
        .ok_or_else(|| AppError::Internal("Stored money is outside f64 range".into()))
}

pub(crate) async fn event(
    connection: &mut SqliteConnection,
    aggregate_type: &str,
    aggregate_id: Uuid,
    event_type: &str,
    location_id: Option<Uuid>,
    deleted: bool,
    payload: Value,
) -> Result<(), AppError> {
    super::tenant_activity::record_canonical_on(connection, event_type, aggregate_type, aggregate_id, location_id, &payload).await?;
    write_outbox_event_on_connection(
        connection,
        NewOutboxEvent {
            location_id,
            aggregate_type: aggregate_type.into(),
            aggregate_id,
            event_type: event_type.into(),
            schema_version: 1,
            deleted,
            payload,
        },
    )
    .await?;
    Ok(())
}

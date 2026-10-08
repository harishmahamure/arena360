//! Tenant backup transport and durable WAL capture (ADR-0043 decision 26).
pub mod batch;
pub mod crypto;
pub mod ledger;
pub mod snapshot;
pub mod wal;
pub mod worker;

pub fn configured_worker(
    pool: Option<sqlx::PgPool>,
    cell: Option<uuid::Uuid>,
    metrics: std::sync::Arc<crate::metrics::Metrics>,
) -> Result<Option<worker::Worker>, crate::error::AppError> {
    use crate::error::AppError;
    let bucket = std::env::var("REPLICATION_BUCKET").ok();
    let keys = std::env::var("REPLICATION_KEY_DIR").ok();
    if bucket.is_none() && keys.is_none() {
        return Ok(None);
    }
    let required = || {
        AppError::Internal(
            "Replication requires bucket, separate durable key mount, control plane and cell ID"
                .into(),
        )
    };
    let mut builder = object_store::aws::AmazonS3Builder::from_env()
        .with_bucket_name(bucket.ok_or_else(required)?);
    if let Ok(endpoint) = std::env::var("REPLICATION_ENDPOINT") {
        builder = builder.with_endpoint(endpoint);
    }
    // Clear-text transport is an explicit local-development opt-in.
    if std::env::var("REPLICATION_ALLOW_HTTP").as_deref() == Ok("true") {
        builder = builder.with_allow_http(true);
    }
    let store = builder
        .build()
        .map_err(|e| AppError::Internal(format!("Replication object storage: {e}")))?;
    Ok(Some(worker::Worker {
        gates: Default::default(),
        store: std::sync::Arc::new(store),
        ledger: std::sync::Arc::new(ledger::PostgresLedger {
            pool: pool.ok_or_else(required)?,
            cell_id: cell.ok_or_else(required)?,
        }),
        keys: crypto::TenantKeys::new(keys.ok_or_else(required)?),
        metrics,
    }))
}

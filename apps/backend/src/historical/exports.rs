//! Durable export admission and signed downloads; independent of tenant hydration.
use crate::{error::AppError, tenancy::analytics_snapshot::TABLES};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Filters {
    pub table: String,
    pub location_id: Option<Uuid>,
    #[serde(default)]
    pub daily_revenue: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    #[serde(deserialize_with = "utc_input")]
    pub start: DateTime<Utc>,
    #[serde(deserialize_with = "utc_input")]
    pub end: DateTime<Utc>,
    pub format: String,
    pub filters: Filters,
}
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Job {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub requested_by: Uuid,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub filters: serde_json::Value,
    pub format: String,
    pub state: String,
    pub source_objects: serde_json::Value,
    pub result_object_key: Option<String>,
    pub checksum_sha256: Option<String>,
    pub result_size_bytes: Option<i64>,
    pub row_count: Option<i64>,
    pub worker_cell: Option<Uuid>,
    pub worker_token: Option<Uuid>,
    pub worker_expires_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub last_error: Option<String>,
    pub result_object: Option<serde_json::Value>,
}
pub fn validate(request: &Request) -> Result<(), AppError> {
    let spec = TABLES
        .iter()
        .find(|t| t.name == request.filters.table)
        .ok_or_else(|| AppError::BadRequest("Unknown export dataset".into()))?;
    if request.end <= request.start
        || request.end - request.start > chrono::Duration::days(366 * 20)
        || !matches!(request.format.as_str(), "CSV" | "CSV_GZ" | "PARQUET")
    {
        return Err(AppError::BadRequest(
            "Choose CSV, CSV_GZ or PARQUET and an ordered range of at most twenty years".into(),
        ));
    }
    if spec.time.is_none() && spec.parent.is_none() {
        return Err(AppError::BadRequest(
            "Select a historical fact dataset".into(),
        ));
    }
    if request.filters.location_id.is_some()
        && !spec.columns.iter().any(|c| c.name == "location_id")
    {
        return Err(AppError::BadRequest(
            "This dataset has no location filter".into(),
        ));
    }
    if request.filters.daily_revenue && request.filters.table != "transactions" {
        return Err(AppError::BadRequest(
            "Daily revenue uses the transactions dataset".into(),
        ));
    }
    Ok(())
}
pub async fn get(pool: &PgPool, tenant: Uuid, id: Uuid) -> Result<Job, AppError> {
    sqlx::query_as("SELECT * FROM historical_exports WHERE id=$1 AND tenant_id=$2")
        .bind(id)
        .bind(tenant)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Export job not found".into()))
}
pub async fn enqueue(
    pool: &PgPool,
    tenant: Uuid,
    user: Uuid,
    request: Request,
) -> Result<Job, AppError> {
    validate(&request)?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO historical_exports(id,tenant_id,requested_by,period_start,period_end,filters,format,expires_at) SELECT $1,$2,$3,$4,$5,$6,$7,clock_timestamp()+INTERVAL '7 days' FROM tenants WHERE id=$2 AND state IN ('ACTIVE','COLD')")
        .bind(id).bind(tenant).bind(user).bind(request.start).bind(request.end).bind(serde_json::to_value(request.filters).map_err(|e|AppError::Internal(e.to_string()))?).bind(request.format).execute(pool).await?;
    get(pool, tenant, id).await
}
pub async fn claim(pool: &PgPool, cell: Uuid) -> Result<Option<Job>, AppError> {
    claim_inner(pool, cell, None).await
}
pub async fn claim_specific(pool: &PgPool, cell: Uuid, id: Uuid) -> Result<Option<Job>, AppError> {
    claim_inner(pool, cell, Some(id)).await
}
async fn claim_inner(
    pool: &PgPool,
    cell: Uuid,
    candidate: Option<Uuid>,
) -> Result<Option<Job>, AppError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('historical-export-admission',0))")
        .execute(&mut *tx)
        .await?;
    let limits:(i32,i32,i32)=sqlx::query_as("SELECT platform_slots,tenant_slots,cell_slots FROM historical_export_limits WHERE singleton FOR UPDATE").fetch_one(&mut *tx).await?;
    let cell_active: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cells WHERE id=$1 AND state='ACTIVE')")
            .bind(cell)
            .fetch_one(&mut *tx)
            .await?;
    if !cell_active {
        return Err(AppError::Forbidden(
            "Export workers require an ACTIVE cell".into(),
        ));
    }
    // Expired owners do not consume slots; tokens fence every subsequent publication.
    let active:(i64,i64)=sqlx::query_as("SELECT COUNT(*),COUNT(*) FILTER(WHERE worker_cell=$1) FROM historical_exports WHERE state NOT IN ('READY','EXPIRED') AND worker_expires_at>clock_timestamp()")
        .bind(cell).fetch_one(&mut *tx).await?;
    if active.0 >= i64::from(limits.0) || active.1 >= i64::from(limits.2) {
        tx.rollback().await?;
        return Ok(None);
    }
    let job:Option<Job>=sqlx::query_as("SELECT e.* FROM historical_exports e JOIN tenants t ON t.id=e.tenant_id WHERE ($2::uuid IS NULL OR e.id=$2) AND e.state IN ('QUEUED','PREPARING','SCANNING_ARCHIVE','GENERATING','UPLOADING') AND e.expires_at>clock_timestamp() AND e.retry_after<=clock_timestamp() AND t.state IN ('ACTIVE','COLD') AND (e.worker_expires_at IS NULL OR e.worker_expires_at<=clock_timestamp()) AND (SELECT COUNT(*) FROM historical_exports other WHERE other.tenant_id=e.tenant_id AND other.state NOT IN ('READY','EXPIRED') AND other.worker_expires_at>clock_timestamp())<$1 ORDER BY e.created_at FOR UPDATE OF e SKIP LOCKED LIMIT 1")
        .bind(limits.1 as i64).bind(candidate).fetch_optional(&mut *tx).await?;
    let Some(job) = job else {
        tx.rollback().await?;
        return Ok(None);
    };
    let token = Uuid::new_v4();
    let claimed:Job=sqlx::query_as("UPDATE historical_exports SET state=CASE WHEN state='QUEUED' THEN 'PREPARING' ELSE state END,started_at=COALESCE(started_at,clock_timestamp()),worker_cell=$2,worker_token=$3,worker_expires_at=clock_timestamp()+INTERVAL '2 minutes',last_error=NULL WHERE id=$1 RETURNING *")
        .bind(job.id).bind(cell).bind(token).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(Some(claimed))
}
pub async fn renew(pool: &PgPool, id: Uuid, token: Uuid) -> Result<(), AppError> {
    let changed=sqlx::query("UPDATE historical_exports SET worker_expires_at=clock_timestamp()+INTERVAL '2 minutes' WHERE id=$1 AND worker_token=$2 AND worker_expires_at>clock_timestamp() AND expires_at>clock_timestamp() AND state IN ('PREPARING','SCANNING_ARCHIVE','GENERATING','UPLOADING') AND EXISTS(SELECT 1 FROM tenants WHERE id=tenant_id AND state IN ('ACTIVE','COLD'))")
        .bind(id).bind(token).execute(pool).await?.rows_affected();
    if changed != 1 {
        return Err(AppError::Forbidden(
            "Export worker token expired or was replaced".into(),
        ));
    }
    Ok(())
}
pub async fn cancel(pool: &PgPool, tenant: Uuid, id: Uuid) -> Result<Job, AppError> {
    let changed=sqlx::query("UPDATE historical_exports SET state='CANCELLED' WHERE id=$1 AND tenant_id=$2 AND state IN ('QUEUED','PREPARING','SCANNING_ARCHIVE','GENERATING','UPLOADING')")
        .bind(id).bind(tenant).execute(pool).await?.rows_affected();
    if changed != 1 {
        return Err(AppError::Conflict(
            "Export cannot be cancelled in its current state".into(),
        ));
    }
    get(pool, tenant, id).await
}

fn utc_input<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<DateTime<Utc>, D::Error> {
    let value = String::deserialize(deserializer)?;
    if !value.ends_with('Z') {
        return Err(serde::de::Error::custom("Use UTC timestamps ending in Z"));
    }
    DateTime::parse_from_rfc3339(&value)
        .map(|at| at.with_timezone(&Utc))
        .map_err(serde::de::Error::custom)
}
pub async fn release(pool: &PgPool, id: Uuid, token: Uuid, error: &str) -> Result<(), AppError> {
    sqlx::query("UPDATE historical_exports SET worker_cell=NULL,worker_token=NULL,worker_expires_at=NULL,last_error=$3,retry_after=clock_timestamp()+INTERVAL '30 seconds' WHERE id=$1 AND worker_token=$2 AND state IN ('PREPARING','SCANNING_ARCHIVE','GENERATING','UPLOADING')")
        .bind(id).bind(token).bind(error).execute(pool).await?;
    Ok(())
}
pub fn sign(secret: &str, tenant: Uuid, id: Uuid, expiry: i64, checksum: &str) -> String {
    use hmac::{Hmac, Mac};
    let mut mac =
        Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key");
    mac.update(format!("historical-download:{tenant}:{id}:{expiry}:{checksum}").as_bytes());
    hex::encode(mac.finalize().into_bytes())
}
pub fn verify_signature(
    secret: &str,
    tenant: Uuid,
    id: Uuid,
    expiry: i64,
    checksum: &str,
    signature: &str,
) -> Result<(), AppError> {
    use hmac::{Hmac, Mac};
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| AppError::Internal("Download signing unavailable".into()))?;
    mac.update(format!("historical-download:{tenant}:{id}:{expiry}:{checksum}").as_bytes());
    let bytes = hex::decode(signature)
        .map_err(|_| AppError::Forbidden("Invalid download signature".into()))?;
    mac.verify_slice(&bytes)
        .map_err(|_| AppError::Forbidden("Invalid download signature".into()))
}

pub fn storage_from_env() -> Result<
    (
        std::sync::Arc<dyn object_store::ObjectStore>,
        crate::replication::crypto::TenantKeys,
    ),
    AppError,
> {
    let keys = crate::replication::crypto::TenantKeys::new(
        std::env::var("REPLICATION_KEY_DIR")
            .map_err(|_| AppError::Internal("Historical storage keys are not configured".into()))?,
    );
    if let Ok(path) = std::env::var("EXPORT_TEST_OBJECT_DIR") {
        if ["NODE_ENV", "RUST_ENV", "ENVIRONMENT"]
            .into_iter()
            .any(|name| std::env::var(name).as_deref() == Ok("production"))
            || std::env::var("RUST_ENV").as_deref() != Ok("test")
        {
            return Err(AppError::Internal(
                "Local export object fixtures require RUST_ENV=test".into(),
            ));
        }
        let store = object_store::local::LocalFileSystem::new_with_prefix(path)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        return Ok((std::sync::Arc::new(store), keys));
    }
    let bucket = std::env::var("REPLICATION_BUCKET")
        .map_err(|_| AppError::Internal("Historical object storage is not configured".into()))?;
    let mut builder = object_store::aws::AmazonS3Builder::from_env().with_bucket_name(bucket);
    if let Ok(endpoint) = std::env::var("REPLICATION_ENDPOINT") {
        builder = builder.with_endpoint(endpoint);
    }
    if std::env::var("REPLICATION_ALLOW_HTTP").as_deref() == Ok("true") {
        builder = builder.with_allow_http(true);
    }
    Ok((
        std::sync::Arc::new(
            builder
                .build()
                .map_err(|e| AppError::Internal(e.to_string()))?,
        ),
        keys,
    ))
}

/// Remove only expired, marked scratch directories owned by this export runner.
pub async fn cleanup_staging(pool: &PgPool, root: &std::path::Path) -> Result<usize, AppError> {
    let mut removed = 0;
    let entries = std::fs::read_dir(root).map_err(|e| AppError::Internal(e.to_string()))?;
    for entry in entries.take(100) {
        let entry = entry.map_err(|e| AppError::Internal(e.to_string()))?;
        if !entry
            .file_type()
            .map_err(|e| AppError::Internal(e.to_string()))?
            .is_dir()
        {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.len() != 73 || name.as_bytes()[36] != b'-' {
            continue;
        }
        let (Ok(id), Ok(token)) = (Uuid::parse_str(&name[..36]), Uuid::parse_str(&name[37..]))
        else {
            continue;
        };
        let path = entry.path();
        let Ok(marker) = std::fs::read(path.join("export-owner.json")) else {
            continue;
        };
        let Ok(marker) = serde_json::from_slice::<serde_json::Value>(&marker) else {
            continue;
        };
        if marker["id"] != id.to_string() || marker["token"] != token.to_string() {
            continue;
        }
        let age = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|at| at.elapsed().ok());
        if age.is_none_or(|age| age < std::time::Duration::from_secs(150)) {
            continue;
        }
        let safe:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM historical_exports WHERE id=$1 AND worker_token=$2 AND worker_expires_at>=clock_timestamp()-INTERVAL '30 seconds' AND state<>'READY')")
            .bind(id).bind(token).fetch_one(pool).await?;
        if !safe {
            continue;
        }
        std::fs::remove_dir_all(path).map_err(|e| AppError::Internal(e.to_string()))?;
        removed += 1;
    }
    Ok(removed)
}

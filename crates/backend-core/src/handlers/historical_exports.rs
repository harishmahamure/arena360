use crate::{
    app::AppState,
    dto::{ApiResult, JwtUserClaims},
    error::AppError,
    historical::{exports, objects},
    middleware::AdminUser,
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderValue},
    response::Response,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;
pub async fn authorize(
    state: &AppState,
    claims: &JwtUserClaims,
) -> Result<(sqlx::PgPool, Uuid), AppError> {
    if !claims.is_admin() || claims.deviceId.is_some() {
        return Err(AppError::Forbidden(
            "Organization admin access required".into(),
        ));
    }
    let pool = state
        .control_db
        .clone()
        .ok_or_else(|| AppError::Internal("Historical export control plane unavailable".into()))?;
    if !crate::middleware::auth::control_panel_session_active(&pool, claims).await? {
        return Err(AppError::Unauthorized(
            "Account or tenant access changed; sign in again".into(),
        ));
    }
    let tenant = claims
        .tenantId
        .parse()
        .map_err(|_| AppError::Unauthorized("Invalid tenant identity".into()))?;
    Ok((pool, tenant))
}
fn status(job: &exports::Job) -> Value {
    json!({"id":job.id,"state":if job.expires_at<=chrono::Utc::now()&&job.state=="READY"{"EXPIRED"}else{&job.state},"format":job.format,"rowCount":job.row_count,"sizeBytes":job.result_size_bytes,"expiresAt":job.expires_at,"error":job.last_error.as_ref().map(|_|"Export is waiting for retry; contact your administrator if it persists")})
}
pub async fn create(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(request): Json<exports::Request>,
) -> ApiResult<Value> {
    let (pool, tenant) = authorize(&state, &claims).await?;
    let user = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))?;
    let job = exports::enqueue(&pool, tenant, user, request).await?;
    crate::dto::ok(status(&job))
}
pub async fn get(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Value> {
    let (pool, tenant) = authorize(&state, &claims).await?;
    crate::dto::ok(status(&exports::get(&pool, tenant, id).await?))
}
pub async fn cancel(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Value> {
    let (pool, tenant) = authorize(&state, &claims).await?;
    crate::dto::ok(status(&exports::cancel(&pool, tenant, id).await?))
}
pub async fn download_link(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Value> {
    let (pool, tenant) = authorize(&state, &claims).await?;
    let job = exports::get(&pool, tenant, id).await?;
    let now: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await?;
    if job.state != "READY" || job.expires_at <= now {
        return Err(AppError::Conflict(
            "Export is not ready or has expired".into(),
        ));
    }
    let expiry = (now + chrono::Duration::minutes(10))
        .timestamp()
        .min(job.expires_at.timestamp());
    let checksum = job
        .checksum_sha256
        .as_deref()
        .ok_or_else(|| AppError::Internal("Export evidence unavailable".into()))?;
    let signature = exports::sign(&state.settings.jwt_secret, tenant, id, expiry, checksum);
    crate::dto::ok(
        json!({"url":format!("/historical/downloads/{tenant}/{id}?expires={expiry}&signature={signature}"),"expiresAt":crate::time::format_sqlite_timestamp(&chrono::DateTime::from_timestamp(expiry,0).unwrap()).map_err(|e|AppError::Internal(e.to_string()))?}),
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownloadQuery {
    pub expires: i64,
    pub signature: String,
}
struct Files(std::path::PathBuf);
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
pub async fn download(
    State(state): State<Arc<AppState>>,
    Path((tenant, id)): Path<(Uuid, Uuid)>,
    Query(query): Query<DownloadQuery>,
) -> Result<Response, AppError> {
    let pool = state
        .control_db
        .as_ref()
        .ok_or_else(|| AppError::Internal("Historical downloads unavailable".into()))?;
    let job = exports::get(pool, tenant, id).await?;
    let now: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(pool)
        .await?;
    let tenant_live: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM tenants t JOIN organization_memberships m ON m.tenant_id=t.id JOIN users u ON u.id=m.user_id WHERE t.id=$1 AND t.state NOT IN ('DELETED','FAILED') AND u.id=$2 AND u.is_active AND u.deleted_at IS NULL AND m.is_active AND m.role='admin')",
    )
    .bind(tenant)
    .bind(job.requested_by)
    .fetch_one(pool)
    .await?;
    if !tenant_live
        || job.state != "READY"
        || query.expires <= now.timestamp()
        || query.expires > job.expires_at.timestamp()
        || query.expires > now.timestamp() + 600
        || query.signature.len() != 64
    {
        return Err(AppError::Forbidden(
            "Download link expired or invalid".into(),
        ));
    }
    let checksum = job
        .checksum_sha256
        .as_deref()
        .ok_or_else(|| AppError::Internal("Export evidence unavailable".into()))?;
    exports::verify_signature(
        &state.settings.jwt_secret,
        tenant,
        id,
        query.expires,
        checksum,
        &query.signature,
    )?;
    let object: objects::Object = serde_json::from_value(
        job.result_object
            .clone()
            .ok_or_else(|| AppError::Internal("Export object unavailable".into()))?,
    )
    .map_err(|e| AppError::Internal(e.to_string()))?;
    if object.plaintext_checksum != checksum
        || job.result_object_key.as_deref() != Some(&object.key)
    {
        return Err(AppError::Internal("Export evidence differs".into()));
    }
    let (store, keys) = exports::storage_from_env()?;
    let key = keys.read(tenant)?;
    let base = state
        .settings
        .tenant_data_dir
        .join("export-staging/downloads");
    std::fs::create_dir_all(&base).map_err(|e| AppError::Internal(e.to_string()))?;
    if std::env::var("DISK_PRESSURE_MONITOR").as_deref() != Ok("false")
        && crate::disk::measure(&base)
            .map_or(true, |s| crate::disk::zone(s.used_permille(), 0) >= 2)
    {
        return Err(AppError::Conflict(
            "Download temporarily unavailable; try again later".into(),
        ));
    }
    static DOWNLOAD_SLOTS: std::sync::LazyLock<Arc<tokio::sync::Semaphore>> =
        std::sync::LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(2)));
    let permit = DOWNLOAD_SLOTS
        .clone()
        .try_acquire_owned()
        .map_err(|_| AppError::Api {
            code: "HISTORICAL_DOWNLOAD_BUSY".into(),
            status: axum::http::StatusCode::TOO_MANY_REQUESTS,
            details: None,
        })?;
    let root = base.join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).map_err(|e| AppError::Internal(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }
    let content_type = match job.format.as_str() {
        "CSV" => "text/csv; charset=utf-8",
        "CSV_GZ" => "application/gzip",
        _ => "application/vnd.apache.parquet",
    };
    let extension = match job.format.as_str() {
        "CSV" => "csv",
        "CSV_GZ" => "csv.gz",
        _ => "parquet",
    };
    let (sender, receiver) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, std::io::Error>>(2);
    tokio::spawn(async move {
        let _permit = permit;
        let _files = Files(root.clone());
        let path = root.join("download");
        let result = async {
            tokio::select!{result=objects::download(store.as_ref(),&key,&object,&path)=>result?,_=sender.closed()=>return Ok(())}
            let mut file = tokio::fs::File::open(path)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
            use tokio::io::AsyncReadExt;
            loop {
                let mut chunk = vec![0u8; 65536];
                let n = file
                    .read(&mut chunk)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                if n == 0 {
                    break;
                }
                chunk.truncate(n);
                if sender.send(Ok(chunk.into())).await.is_err() {
                    break;
                }
            }
            Ok::<_, AppError>(())
        }
        .await;
        if let Err(error) = result {
            tracing::warn!(%id,%error,"Historical download interrupted");
            let _ = sender
                .send(Err(std::io::Error::other("Download interrupted")))
                .await;
        }
    });
    let stream = tokio_stream::wrappers::ReceiverStream::new(receiver);
    let mut response = Response::new(Body::from_stream(stream));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"arena360-{id}.{extension}\""
        ))
        .map_err(|e| AppError::Internal(e.to_string()))?,
    );
    if let Some(bytes) = job.result_size_bytes {
        response.headers_mut().insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&bytes.to_string())
                .map_err(|e| AppError::Internal(e.to_string()))?,
        );
    }
    Ok(response)
}

/// These new control-plane endpoints remain available when legacy business REST is disabled.
pub fn router(state: Arc<AppState>) -> axum::Router {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/historical/exports", post(create))
        .route("/historical/exports/{id}", get(self::get).delete(cancel))
        .route("/historical/exports/{id}/download", get(download_link))
        .route("/historical/downloads/{tenant}/{id}", get(download))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::auth::auth_middleware,
        ))
        .with_state(state)
}

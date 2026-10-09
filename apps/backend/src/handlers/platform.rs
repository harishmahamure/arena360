//! Operator-only control-plane surface, independent of tenant JWTs and routing.
use crate::{
    app::AppState,
    control::{CreateTenant, Repository},
    error::AppError,
    tenancy::{ProvisionTenant, TenantProvisioner},
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Sha256;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct Context {
    pub pool: Option<PgPool>,
    pub provisioner: Option<Arc<TenantProvisioner>>,
    pub cell: Option<Uuid>,
    pub token: Option<String>,
}
pub fn router(app: Arc<AppState>) -> Router {
    router_with_context(Context {
        pool: app.control_db.clone(),
        provisioner: app.tenant_provisioner.clone(),
        cell: app.settings.cell_id,
        token: std::env::var("PLATFORM_ADMIN_TOKEN")
            .ok()
            .filter(|v| v.len() >= 32 && v.trim() == v),
    })
}
pub fn router_with_context(context: Context) -> Router {
    Router::new()
        .route("/platform/overview", get(overview))
        .route("/platform/cells", get(cells).post(register_cell))
        .route("/platform/tenants", get(tenants).post(create_tenant))
        .route("/platform/tenants/{id}", get(detail))
        .route("/platform/tenants/{id}/timezone", put(timezone))
        .route("/platform/tenants/{id}/admins", post(create_admin))
        .route("/platform/tenants/{id}/move", post(move_tenant))
        .route("/platform/tenants/{id}/cold", post(cool))
        .route("/platform/tenants/{id}/wake", post(wake))
        .route("/platform/tenants/{id}/jobs/{job}/cancel", post(cancel_job))
        .layer(middleware::from_fn_with_state(
            context.clone(),
            authenticate,
        ))
        .with_state(context)
}
fn authorize(expected: Option<&str>, headers: &HeaderMap) -> Result<(), AppError> {
    let expected = expected.filter(|t| t.len() >= 32).ok_or_else(|| {
        unavailable(
            "Platform portal is disabled; configure PLATFORM_ADMIN_TOKEN (at least 32 characters)",
        )
    })?;
    let supplied = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    // HMAC verification compares fixed-size authenticators in constant time.
    let mut candidate =
        Hmac::<Sha256>::new_from_slice(supplied.as_bytes()).expect("HMAC accepts any key");
    candidate.update(b"arena360-platform-operator");
    let mut verifier =
        Hmac::<Sha256>::new_from_slice(expected.as_bytes()).expect("HMAC accepts any key");
    verifier.update(b"arena360-platform-operator");
    verifier
        .verify_slice(&candidate.finalize().into_bytes())
        .map_err(|_| AppError::Unauthorized("Invalid platform operator token".into()))
}
async fn authenticate(
    State(c): State<Context>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, AppError> {
    authorize(c.token.as_deref(), request.headers())?;
    Ok(next.run(request).await)
}
fn unavailable(message: &str) -> AppError {
    AppError::Api {
        code: "PLATFORM_UNAVAILABLE".into(),
        status: StatusCode::SERVICE_UNAVAILABLE,
        details: Some(json!({"message":message})),
    }
}
fn pool(c: &Context) -> Result<&PgPool, AppError> {
    c.pool
        .as_ref()
        .ok_or_else(|| unavailable("Control database is not configured"))
}
fn text(value: &str, field: &str, max: usize) -> Result<(), AppError> {
    if value.trim().is_empty() || value.len() > max || value.trim() != value {
        return Err(AppError::BadRequest(format!(
            "{field} must be 1..{max} characters with no surrounding whitespace"
        )));
    }
    Ok(())
}
async fn overview(State(c): State<Context>) -> Result<Json<Value>, AppError> {
    let counts: Value = sqlx::query_scalar("SELECT jsonb_build_object('total',count(*),'active',count(*) FILTER(WHERE state='ACTIVE'),'cold',count(*) FILTER(WHERE state='COLD'),'attention',count(*) FILTER(WHERE state NOT IN ('ACTIVE','COLD','DELETED'))) FROM tenants WHERE state<>'DELETED'").fetch_one(pool(&c)?).await?;
    Ok(Json(
        json!({"counts":counts,"localCellId":c.cell,"canProvision":c.provisioner.is_some(),"targetSchemaVersion":crate::tenancy::target_schema_version()}),
    ))
}
async fn cells(State(c): State<Context>) -> Result<Json<Value>, AppError> {
    let rows: Vec<Value> = sqlx::query_scalar("SELECT to_jsonb(x) FROM (SELECT c.id,c.name,c.address,c.state,c.hydration_heartbeat_at,(c.hydration_heartbeat_at>clock_timestamp()-INTERVAL '30 seconds') AS hydration_ready,(SELECT count(*) FROM tenants t WHERE t.owner_cell=c.id AND t.state<>'DELETED') AS tenant_count FROM cells c ORDER BY c.name LIMIT 500) x").fetch_all(pool(&c)?).await?;
    Ok(Json(json!(rows)))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NewCell {
    id: Option<Uuid>,
    name: String,
    address: String,
}
async fn register_cell(
    State(c): State<Context>,
    Json(input): Json<NewCell>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    text(&input.name, "name", 100)?;
    let url = reqwest::Url::parse(&input.address)
        .map_err(|_| AppError::BadRequest("Cell address must be an HTTP(S) origin".into()))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err(AppError::BadRequest(
            "Cell address must be an HTTP(S) origin without credentials, path or query".into(),
        ));
    }
    let id = input.id.unwrap_or_else(Uuid::new_v4);
    let row: Value=sqlx::query_scalar("INSERT INTO cells(id,name,address) VALUES($1,$2,$3) RETURNING jsonb_build_object('id',id,'name',name,'address',address,'state',state)").bind(id).bind(input.name).bind(input.address.trim_end_matches('/')).fetch_one(pool(&c)?).await.map_err(conflict)?;
    Ok((StatusCode::CREATED, Json(row)))
}
fn conflict(e: sqlx::Error) -> AppError {
    if e.as_database_error()
        .is_some_and(|e| e.is_unique_violation())
    {
        AppError::Conflict("That identifier or name already exists".into())
    } else {
        e.into()
    }
}
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct List {
    search: Option<String>,
    state: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
}
async fn tenants(State(c): State<Context>, Query(q): Query<List>) -> Result<Json<Value>, AppError> {
    let search = q.search.unwrap_or_default();
    if search.len() > 100 {
        return Err(AppError::BadRequest("Search is too long".into()));
    }
    let offset = q.offset.unwrap_or(0);
    let limit = q.limit.unwrap_or(30);
    if offset < 0 || !(1..=100).contains(&limit) {
        return Err(AppError::BadRequest("Invalid page".into()));
    }
    let rows:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(x) FROM (SELECT t.id,t.slug,t.name,t.state,t.timezone,t.owner_cell,t.ownership_generation,t.schema_version,t.created_at,c.name AS cell_name,l.expires_at AS lease_expires_at,(l.expires_at>clock_timestamp()) AS lease_fresh FROM tenants t LEFT JOIN cells c ON c.id=t.owner_cell LEFT JOIN tenant_leases l ON l.tenant_id=t.id AND l.owner_cell=t.owner_cell AND l.ownership_generation=t.ownership_generation WHERE ($1='' OR position(lower($1) IN lower(t.name||' '||t.slug||' '||t.id::text))>0) AND ($2::text IS NULL OR t.state=$2) ORDER BY t.created_at DESC,t.id LIMIT $3 OFFSET $4) x").bind(search).bind(q.state.filter(|s| !s.is_empty())).bind(limit).bind(offset).fetch_all(pool(&c)?).await?;
    Ok(Json(json!({"items":rows,"offset":offset,"limit":limit})))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NewTenant {
    slug: String,
    name: String,
    timezone: String,
    trial_days: i64,
}
async fn create_tenant(
    State(c): State<Context>,
    Json(input): Json<NewTenant>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    text(&input.name, "name", 150)?;
    text(&input.slug, "slug", 63)?;
    if !input
        .slug
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || input.slug.starts_with('-')
        || input.slug.ends_with('-')
        || !(1..=365).contains(&input.trial_days)
    {
        return Err(AppError::BadRequest(
            "Use a lowercase slug and a trial of 1..365 days".into(),
        ));
    }
    let provisioner = c
        .provisioner
        .as_ref()
        .ok_or_else(|| unavailable("Provisioning requires a configured local cell"))?;
    let tenant = provisioner
        .provision(ProvisionTenant {
            tenant: CreateTenant {
                slug: input.slug,
                name: input.name,
                timezone: input.timezone,
                owner_cell: c.cell,
                subscription_plan: "trial".into(),
                entitlements: json!({}),
                trial_ends_at: Utc::now() + Duration::days(input.trial_days),
                entitlement_grace_until: Utc::now() + Duration::days(input.trial_days + 7),
            },
            settings: vec![],
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(tenant.tenant).map_err(|e| AppError::Internal(e.to_string()))?),
    ))
}
async fn detail(State(c): State<Context>, Path(id): Path<Uuid>) -> Result<Json<Value>, AppError> {
    let p = pool(&c)?;
    let tenant:Value=sqlx::query_scalar("SELECT to_jsonb(x) FROM (SELECT t.id,t.slug,t.name,t.state,t.timezone,t.owner_cell,t.ownership_generation,t.schema_version,t.created_at,c.name AS cell_name,l.expires_at AS lease_expires_at,(l.expires_at>clock_timestamp()) AS lease_fresh FROM tenants t LEFT JOIN cells c ON c.id=t.owner_cell LEFT JOIN tenant_leases l ON l.tenant_id=t.id AND l.owner_cell=t.owner_cell AND l.ownership_generation=t.ownership_generation WHERE t.id=$1) x").bind(id).fetch_optional(p).await?.ok_or_else(||AppError::NotFound("Tenant not found".into()))?;
    let mut jobs = vec![];
    // Fixed SQL table names only. Do not expose object keys, worker tokens or user credentials.
    for (table, phase, kind) in [
        ("tenant_moves", "phase", "Move"),
        ("tenant_cold_jobs", "phase", "Cold lifecycle"),
        ("archive_manifests", "state", "Archive"),
        ("historical_backfills", "state", "Backfill"),
        ("historical_exports", "state", "Export"),
    ] {
        let sql=format!("SELECT jsonb_build_object('id',id,'kind',$2::text,'state',{phase},'created_at',created_at,'last_error',last_error) FROM {table} WHERE tenant_id=$1 ORDER BY created_at DESC LIMIT 10");
        jobs.extend(
            sqlx::query_scalar::<_, Value>(&sql)
                .bind(id)
                .bind(kind)
                .fetch_all(p)
                .await?,
        );
    }
    jobs.sort_by(|a, b| b["created_at"].as_str().cmp(&a["created_at"].as_str()));
    let admins:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',u.id,'username',u.username,'active',u.is_active AND m.is_active) FROM organization_memberships m JOIN users u ON u.id=m.user_id WHERE m.tenant_id=$1 AND m.role='admin' AND u.deleted_at IS NULL ORDER BY u.username").bind(id).fetch_all(p).await?;
    let licenses:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('status',status,'revision',revision,'valid_until',valid_until,'grace_until',grace_until,'entitlements',entitlements) FROM licenses WHERE tenant_id=$1 ORDER BY revision DESC LIMIT 5").bind(id).fetch_all(p).await?;
    Ok(Json(
        json!({"tenant":tenant,"jobs":jobs,"admins":admins,"licenses":licenses}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Zone {
    timezone: String,
}
async fn timezone(
    State(c): State<Context>,
    Path(id): Path<Uuid>,
    Json(input): Json<Zone>,
) -> Result<Json<Value>, AppError> {
    let t = Repository::new(pool(&c)?.clone())
        .update_timezone(id, &input.timezone)
        .await?;
    Ok(Json(json!(t)))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Admin {
    username: String,
    password: String,
}
async fn create_admin(
    State(c): State<Context>,
    Path(id): Path<Uuid>,
    Json(input): Json<Admin>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    text(&input.username, "username", 100)?;
    if !(12..=72).contains(&input.password.len()) {
        return Err(AppError::BadRequest("Password must be 12..72 bytes".into()));
    }
    let hash =
        tokio::task::spawn_blocking(move || bcrypt::hash(input.password, bcrypt::DEFAULT_COST))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
            .map_err(|e| AppError::Internal(e.to_string()))?;
    let mut tx = pool(&c)?.begin().await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM tenants WHERE id=$1 AND state NOT IN ('FAILED','DELETED'))",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if !exists {
        return Err(AppError::NotFound("Tenant not found".into()));
    }
    let user: Uuid =
        sqlx::query_scalar("INSERT INTO users(username,password_hash) VALUES($1,$2) RETURNING id")
            .bind(&input.username)
            .bind(hash)
            .fetch_one(&mut *tx)
            .await
            .map_err(conflict)?;
    sqlx::query(
        "INSERT INTO organization_memberships(tenant_id,user_id,role) VALUES($1,$2,'admin')",
    )
    .bind(id)
    .bind(user)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id":user,"username":input.username})),
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Target {
    target_cell: Uuid,
}
async fn move_tenant(
    State(c): State<Context>,
    Path(id): Path<Uuid>,
    Json(input): Json<Target>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((
        StatusCode::ACCEPTED,
        Json(json!(
            crate::moving::control::enqueue(pool(&c)?, id, input.target_cell).await?
        )),
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Idle {
    minimum_idle_seconds: i64,
}
async fn cool(
    State(c): State<Context>,
    Path(id): Path<Uuid>,
    Json(input): Json<Idle>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((
        StatusCode::ACCEPTED,
        Json(json!(
            crate::cold::enqueue(pool(&c)?, id, input.minimum_idle_seconds).await?
        )),
    ))
}
async fn wake(
    State(c): State<Context>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let coordinator = crate::cold::Coordinator {
        pool: pool(&c)?.clone(),
        wait_limit: std::time::Duration::from_secs(1),
    };
    match coordinator.wake(id).await {
        Ok(()) => Ok((StatusCode::OK, Json(json!({"state":"ACTIVE"})))),
        Err(AppError::Api { code, .. }) if code == "TENANT_HYDRATION_PENDING" => {
            let state: String = sqlx::query_scalar("SELECT state FROM tenants WHERE id=$1")
                .bind(id)
                .fetch_one(pool(&c)?)
                .await?;
            if state == "RESTORING" {
                Ok((
                    StatusCode::ACCEPTED,
                    Json(
                        json!({"state":state,"message":"Hydration requested; refresh to follow progress"}),
                    ),
                ))
            } else {
                Err(AppError::Conflict("Hydration is not ready; verify a live hydration-capable cell and completed source cleanup".into()))
            }
        }
        Err(e) => Err(e),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operator_token_is_separate_and_required() {
        let token = "test-platform-operator-token-with-32-characters";
        let mut h = HeaderMap::new();
        assert!(authorize(None, &h).is_err());
        assert!(authorize(Some("short"), &h).is_err());
        assert!(authorize(Some(token), &h).is_err());
        h.insert("authorization", "Bearer tenant-jwt".parse().unwrap());
        assert!(authorize(Some(token), &h).is_err());
        h.insert("authorization", format!("Bearer {token}").parse().unwrap());
        assert!(authorize(Some(token), &h).is_ok());
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cancel {
    kind: String,
}
async fn cancel_job(
    State(c): State<Context>,
    Path((tenant, id)): Path<(Uuid, Uuid)>,
    Json(input): Json<Cancel>,
) -> Result<Json<Value>, AppError> {
    let p = pool(&c)?;
    match input.kind.as_str() {
        "Move" => {
            let job = crate::moving::control::get(p, id).await?;
            if job.tenant_id != tenant {
                return Err(AppError::NotFound("Job not found for this tenant".into()));
            }
            crate::moving::control::cancel(p, id).await?;
        }
        "Cold lifecycle" => {
            let job = crate::cold::get(p, id).await?;
            if job.tenant_id != tenant {
                return Err(AppError::NotFound("Job not found for this tenant".into()));
            }
            crate::cold::cancel(p, id).await?;
        }
        _ => {
            return Err(AppError::BadRequest(
                "Only pending move and unreleased cold jobs can be cancelled here".into(),
            ))
        }
    }
    Ok(Json(json!({"state":"CANCELLED"})))
}

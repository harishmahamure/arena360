//! Operator-only control-plane surface, independent of tenant JWTs and routing.
use crate::{
    app::AppState,
    control::{CreateTenant, Repository},
    error::AppError,
    tenancy::{ProvisionTenant, TenantProvisioner},
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    middleware,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct Context {
    pub pool: Option<PgPool>,
    pub provisioner: Option<Arc<TenantProvisioner>>,
    pub cell: Option<Uuid>,
    pub remote_storage: Option<crate::storage_rpc::RemoteStorage>,
}
pub fn router(app: Arc<AppState>) -> Router {
    router_with_context(Context {
        pool: app.control_db.clone(),
        provisioner: app.tenant_provisioner.clone(),
        cell: app
            .settings
            .cell_id
            .or_else(|| app.remote_storage.as_ref().map(|remote| remote.cell_id)),
        remote_storage: app.remote_storage.clone(),
    })
}
pub fn router_with_context(context: Context) -> Router {
    let protected = Router::new()
        .route("/platform/overview", get(overview))
        .route("/platform/cells", get(cells).post(register_cell))
        .route("/platform/plans", get(plans).post(create_plan))
        .route("/platform/plans/{code}", put(update_plan))
        .route("/platform/tenants", get(tenants).post(create_tenant))
        .route("/platform/tenants/{id}", get(detail))
        .route("/platform/tenants/{id}/timezone", put(timezone))
        .route(
            "/platform/tenants/{id}/subscription",
            put(update_subscription),
        )
        .route("/platform/tenants/{id}/enabled", put(set_enabled))
        .route("/platform/tenants/{id}/admins", post(create_admin))
        .route("/platform/tenants/{id}/move", post(move_tenant))
        .route("/platform/tenants/{id}/cold", post(cool))
        .route("/platform/tenants/{id}/wake", post(wake))
        .route("/platform/tenants/{id}/jobs/{job}/cancel", post(cancel_job))
        .layer(middleware::from_fn_with_state(
            context.clone(),
            platform_auth::authenticate,
        ));
    protected.merge(platform_auth::router()).with_state(context)
}
mod platform_auth;
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
    let counts: Value = sqlx::query_scalar("SELECT jsonb_build_object('total',count(*),'active',count(*) FILTER(WHERE state='ACTIVE' AND is_enabled AND EXISTS(SELECT 1 FROM subscriptions s WHERE s.tenant_id=tenants.id AND s.status IN ('TRIAL','ACTIVE') AND s.ends_at>clock_timestamp())),'cold',count(*) FILTER(WHERE state='COLD'),'attention',count(*) FILTER(WHERE state NOT IN ('ACTIVE','COLD','DELETED') OR NOT is_enabled OR NOT EXISTS(SELECT 1 FROM subscriptions s WHERE s.tenant_id=tenants.id AND s.status IN ('TRIAL','ACTIVE') AND s.ends_at>clock_timestamp()))) FROM tenants WHERE state<>'DELETED'").fetch_one(pool(&c)?).await?;
    let address = if let Some(remote) = &c.remote_storage {
        Some(remote.address.clone())
    } else {
        sqlx::query_scalar::<_, String>("SELECT address FROM cells WHERE id=$1")
            .bind(c.cell)
            .fetch_optional(pool(&c)?)
            .await?
    };
    Ok(Json(
        json!({"counts":counts,"localCellId":c.cell,"canProvision":c.provisioner.is_some() || c.remote_storage.is_some(),"targetSchemaVersion":crate::tenancy::target_schema_version(),"provisioningAddress":address,"provisioningMode":if c.remote_storage.is_some(){"remote"}else if c.provisioner.is_some(){"local"}else{"unavailable"}}),
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
    let rows:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(x) FROM (SELECT t.id,t.slug,t.name,t.state,t.is_enabled,t.timezone,t.owner_cell,t.ownership_generation,t.schema_version,t.created_at,c.name AS cell_name,l.expires_at AS lease_expires_at,(l.expires_at>clock_timestamp()) AS lease_fresh FROM tenants t LEFT JOIN cells c ON c.id=t.owner_cell LEFT JOIN tenant_leases l ON l.tenant_id=t.id AND l.owner_cell=t.owner_cell AND l.ownership_generation=t.ownership_generation WHERE ($1='' OR position(lower($1) IN lower(t.name||' '||t.slug||' '||t.id::text))>0) AND ($2::text IS NULL OR t.state=$2) ORDER BY t.created_at DESC,t.id LIMIT $3 OFFSET $4) x").bind(search).bind(q.state.filter(|s| !s.is_empty())).bind(limit).bind(offset).fetch_all(pool(&c)?).await?;
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
    if c.provisioner.is_none() && c.remote_storage.is_none() {
        return Err(unavailable(
            "Provisioning requires a configured storage cell",
        ));
    }
    let trial_plan: (Value, i32) = sqlx::query_as(
        "SELECT entitlements,grace_days FROM platform_plans WHERE code='trial' AND is_active",
    )
    .fetch_optional(pool(&c)?)
    .await?
    .ok_or_else(|| {
        AppError::Conflict("Activate the trial plan before provisioning tenants".into())
    })?;
    let trial_end = Utc::now() + Duration::days(input.trial_days);
    let request = ProvisionTenant {
        tenant: CreateTenant {
            slug: input.slug,
            name: input.name,
            timezone: input.timezone,
            owner_cell: c.cell,
            subscription_plan: "trial".into(),
            entitlements: trial_plan.0,
            trial_ends_at: trial_end,
            entitlement_grace_until: trial_end + Duration::days(i64::from(trial_plan.1)),
        },
        settings: vec![],
    };
    let tenant = if let Some(remote) = &c.remote_storage {
        remote.client.provision(&remote.address, &request).await?
    } else {
        let tenant = c
            .provisioner
            .as_ref()
            .expect("checked provisioner")
            .provision(request)
            .await?;
        serde_json::to_value(tenant.tenant).map_err(|e| AppError::Internal(e.to_string()))?
    };
    Ok((StatusCode::CREATED, Json(tenant)))
}
async fn detail(State(c): State<Context>, Path(id): Path<Uuid>) -> Result<Json<Value>, AppError> {
    let p = pool(&c)?;
    let tenant:Value=sqlx::query_scalar("SELECT to_jsonb(x) FROM (SELECT t.id,t.slug,t.name,t.state,t.is_enabled,t.timezone,t.owner_cell,t.ownership_generation,t.schema_version,t.created_at,c.name AS cell_name,l.expires_at AS lease_expires_at,(l.expires_at>clock_timestamp()) AS lease_fresh FROM tenants t LEFT JOIN cells c ON c.id=t.owner_cell LEFT JOIN tenant_leases l ON l.tenant_id=t.id AND l.owner_cell=t.owner_cell AND l.ownership_generation=t.ownership_generation WHERE t.id=$1) x").bind(id).fetch_optional(p).await?.ok_or_else(||AppError::NotFound("Tenant not found".into()))?;
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
    let subscription:Option<Value>=sqlx::query_scalar("SELECT jsonb_build_object('planCode',plan_code,'status',status,'startsAt',starts_at,'endsAt',ends_at) FROM subscriptions WHERE tenant_id=$1 AND status IN ('TRIAL','ACTIVE','PAST_DUE','SUSPENDED') ORDER BY created_at DESC LIMIT 1").bind(id).fetch_optional(p).await?;
    Ok(Json(
        json!({"tenant":tenant,"jobs":jobs,"admins":admins,"licenses":licenses,"subscription":subscription}),
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubscriptionUpdate {
    plan_code: String,
    ends_at: DateTime<Utc>,
}
async fn update_subscription(
    State(c): State<Context>,
    Path(id): Path<Uuid>,
    Json(input): Json<SubscriptionUpdate>,
) -> Result<Json<Value>, AppError> {
    text(&input.plan_code, "plan code", 64)?;
    if !input
        .plan_code
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || input.ends_at <= Utc::now()
    {
        return Err(AppError::BadRequest(
            "Plan code or future expiry is invalid".into(),
        ));
    }
    let mut tx = pool(&c)?.begin().await?;
    let enabled:Option<bool>=sqlx::query_scalar("SELECT is_enabled FROM tenants WHERE id=$1 AND state NOT IN ('FAILED','DELETED') FOR UPDATE")
        .bind(id).fetch_optional(&mut *tx).await?;
    let enabled = enabled.ok_or_else(|| AppError::NotFound("Tenant not found".into()))?;
    let plan: Option<(Value, i32)> = sqlx::query_as(
        "SELECT entitlements,grace_days FROM platform_plans WHERE code=$1 AND is_active",
    )
    .bind(&input.plan_code)
    .fetch_optional(&mut *tx)
    .await?;
    let (entitlements, grace_days) =
        plan.ok_or_else(|| AppError::BadRequest("Select an active plan".into()))?;
    let grace_until = input.ends_at + Duration::days(i64::from(grace_days));
    let changed=sqlx::query("UPDATE subscriptions SET plan_code=$2,status=CASE WHEN $3 THEN 'ACTIVE' ELSE 'SUSPENDED' END,ends_at=$4,updated_at=clock_timestamp() WHERE tenant_id=$1 AND status IN ('TRIAL','ACTIVE','PAST_DUE','SUSPENDED')")
        .bind(id).bind(&input.plan_code).bind(enabled).bind(input.ends_at).execute(&mut *tx).await?;
    if changed.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "Tenant has no current subscription".into(),
        ));
    }
    let revision: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(revision),0)+1 FROM licenses WHERE tenant_id=$1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query("INSERT INTO licenses(tenant_id,status,revision,entitlements,valid_from,valid_until,grace_until) VALUES($1,$2,$3,$4,clock_timestamp(),$5,$6)")
        .bind(id).bind(if enabled {"ACTIVE"} else {"SUSPENDED"}).bind(revision).bind(entitlements)
        .bind(input.ends_at).bind(grace_until).execute(&mut *tx).await?;
    sqlx::query("SELECT pg_notify($1,$2)")
        .bind(crate::routing::ROUTING_CHANGED_CHANNEL)
        .bind(id.to_string())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"revision":revision,"planCode":input.plan_code,"endsAt":input.ends_at}),
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanInput {
    name: String,
    entitlements: Value,
    grace_days: i32,
    is_active: bool,
}
fn validate_plan(code: &str, input: &PlanInput) -> Result<(), AppError> {
    text(code, "plan code", 64)?;
    text(&input.name, "plan name", 100)?;
    if !code
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || !input.entitlements.is_object()
        || !(0..=30).contains(&input.grace_days)
    {
        return Err(AppError::BadRequest(
            "Invalid plan code, entitlements, or grace days".into(),
        ));
    }
    Ok(())
}
async fn plans(State(c): State<Context>) -> Result<Json<Value>, AppError> {
    let rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('code',code,'name',name,'entitlements',entitlements,'graceDays',grace_days,'isActive',is_active) FROM platform_plans ORDER BY code")
        .fetch_all(pool(&c)?).await?;
    Ok(Json(json!(rows)))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NewPlan {
    code: String,
    name: String,
    entitlements: Value,
    grace_days: i32,
    is_active: bool,
}
async fn create_plan(
    State(c): State<Context>,
    Json(input): Json<NewPlan>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let plan = PlanInput {
        name: input.name,
        entitlements: input.entitlements,
        grace_days: input.grace_days,
        is_active: input.is_active,
    };
    validate_plan(&input.code, &plan)?;
    sqlx::query("INSERT INTO platform_plans(code,name,entitlements,grace_days,is_active) VALUES($1,$2,$3,$4,$5)")
        .bind(&input.code).bind(&plan.name).bind(&plan.entitlements).bind(plan.grace_days).bind(plan.is_active)
        .execute(pool(&c)?).await.map_err(conflict)?;
    Ok((StatusCode::CREATED, Json(json!({"code":input.code}))))
}
async fn update_plan(
    State(c): State<Context>,
    Path(code): Path<String>,
    Json(input): Json<PlanInput>,
) -> Result<Json<Value>, AppError> {
    validate_plan(&code, &input)?;
    let changed=sqlx::query("UPDATE platform_plans SET name=$2,entitlements=$3,grace_days=$4,is_active=$5,updated_at=clock_timestamp() WHERE code=$1")
        .bind(&code).bind(&input.name).bind(&input.entitlements).bind(input.grace_days).bind(input.is_active)
        .execute(pool(&c)?).await?;
    if changed.rows_affected() == 0 {
        return Err(AppError::NotFound("Plan not found".into()));
    }
    Ok(Json(json!({"code":code})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Enabled {
    enabled: bool,
}
async fn set_enabled(
    State(c): State<Context>,
    Path(id): Path<Uuid>,
    Json(input): Json<Enabled>,
) -> Result<Json<Value>, AppError> {
    let mut tx = pool(&c)?.begin().await?;
    let current:Option<bool>=sqlx::query_scalar("SELECT is_enabled FROM tenants WHERE id=$1 AND state NOT IN ('FAILED','DELETED') FOR UPDATE")
        .bind(id).fetch_optional(&mut *tx).await?;
    let current = current.ok_or_else(|| AppError::NotFound("Tenant not found".into()))?;
    if current != input.enabled {
        if input.enabled {
            let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM subscriptions WHERE tenant_id=$1 AND status IN ('TRIAL','ACTIVE','SUSPENDED','PAST_DUE') AND ends_at>clock_timestamp())")
                .bind(id).fetch_one(&mut *tx).await?;
            if !valid {
                return Err(AppError::Conflict(
                    "Extend the subscription before enabling this tenant".into(),
                ));
            }
        }
        sqlx::query("UPDATE tenants SET is_enabled=$2,updated_at=clock_timestamp() WHERE id=$1")
            .bind(id)
            .bind(input.enabled)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE subscriptions SET status=CASE WHEN $2 THEN 'ACTIVE' ELSE 'SUSPENDED' END,updated_at=clock_timestamp() WHERE tenant_id=$1 AND status IN ('TRIAL','ACTIVE','SUSPENDED','PAST_DUE')")
            .bind(id).bind(input.enabled).execute(&mut *tx).await?;
        let revision: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(revision),0)+1 FROM licenses WHERE tenant_id=$1",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO licenses(tenant_id,status,revision,entitlements,valid_from,valid_until,grace_until) SELECT tenant_id,$2,$3,entitlements,clock_timestamp(),GREATEST(valid_until,clock_timestamp()+INTERVAL '1 second'),GREATEST(grace_until,clock_timestamp()+INTERVAL '1 second') FROM licenses WHERE tenant_id=$1 ORDER BY revision DESC LIMIT 1")
            .bind(id).bind(if input.enabled {"ACTIVE"} else {"SUSPENDED"}).bind(revision).execute(&mut *tx).await?;
        sqlx::query("SELECT pg_notify($1,$2)")
            .bind(crate::routing::ROUTING_CHANGED_CHANNEL)
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"enabled":input.enabled})))
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

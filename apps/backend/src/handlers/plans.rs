use crate::access::scope::{requested_location, LocationScope};
use crate::services::catalog_scope::{self, CatalogCreate};
use axum::http::HeaderMap;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::error::AppError;
use crate::middleware::{AdminUser, AuthUser};
use crate::models::{CreatePlanDto, Plan, PlanFilterDto, UpdatePlanDto};
use crate::openapi::responses::{
    ActivePlansEnvelope, ErrorEnvelope, PlanEnvelope, PlanPaginationEnvelope,
};
use crate::repositories::TenantPlanRepository;

async fn authorize_tenant_plan(
    db: Arc<crate::tenancy::TenantDb>,
    id: Uuid,
    scope: &LocationScope,
    write: bool,
) -> Result<(), AppError> {
    let (all, rows) = TenantPlanRepository::new(db).location_scope(id).await?;
    let locations = if all {
        vec![]
    } else {
        rows.into_iter()
            .map(|(location_id, _)| location_id)
            .collect()
    };
    catalog_scope::authorize(scope, &locations, write)
}

#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlanLocationQuery {
    pub location_id: Option<Uuid>,
}

async fn pricing_location(
    state: &AppState,
    claims: &crate::dto::JwtUserClaims,
    requested: Option<Uuid>,
) -> Result<Uuid, AppError> {
    if claims.is_admin_or_staff() {
        let org = Uuid::parse_str(&claims.tenantId)
            .map_err(|_| AppError::Forbidden("Select an organization".into()))?;
        let user = claims
            .user_id_uuid()
            .ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))?;
        let location = LocationScope::resolve(&state.db, claims, "plans:read", requested)
            .await?
            .first()?;
        state
            .config
            .ensure_location_permission(org, location, user, "plans:read")
            .await?;
        Ok(location)
    } else if claims.is_device() {
        Ok(state
            .devices
            .get_by_id(
                claims
                    .user_id_uuid()
                    .ok_or_else(|| AppError::Unauthorized("Invalid device".into()))?,
            )
            .await?
            .location_id)
    } else if let Some(device_id) = claims.deviceId.as_deref() {
        let device_id = Uuid::parse_str(device_id)
            .map_err(|_| AppError::Unauthorized("Invalid device identity".into()))?;
        Ok(state.devices.get_by_id(device_id).await?.location_id)
    } else {
        Ok(crate::models::DEFAULT_VENUE_LOCATION_ID)
    }
}

#[utoipa::path(
    get,
    path = "/plans",
    responses(
        (status = 200, description = "List plans", body = PlanPaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "plans"
)]
pub async fn list_plans(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Query(mut filters): Query<PlanFilterDto>,
    headers: HeaderMap,
) -> ApiResult<crate::dto::PaginationResult<Plan>> {
    let scope = LocationScope::resolve(
        &state.db,
        &claims,
        "plans:read",
        filters.location_id.or(requested_location(&headers)?),
    )
    .await?;
    filters.location_id = filters
        .location_id
        .or(requested_location(&headers)?)
        .or_else(|| (scope.locations.len() == 1).then(|| scope.locations[0]));
    filters.organization_id = Some(scope.organization_id);
    filters.allowed_location_ids = Some(scope.locations);
    let result = if let Some(db) = state.tenant_db(scope.organization_id).await? {
        state.plans.list_tenant(db, filters).await?
    } else {
        state.plans.list(filters).await?
    };
    ok(result)
}

#[utoipa::path(
    get,
    path = "/plans/active",
    responses(
        (status = 200, description = "List active plans", body = ActivePlansEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "plans"
)]
pub async fn get_active_plans(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<PlanLocationQuery>,
    headers: HeaderMap,
) -> ApiResult<Vec<Plan>> {
    let location = pricing_location(
        &state,
        &claims,
        query.location_id.or(requested_location(&headers)?),
    )
    .await?;
    let tenant_id = Uuid::parse_str(&claims.tenantId)
        .map_err(|_| AppError::Forbidden("Select an organization".into()))?;
    let plans = if let Some(db) = state.tenant_db(tenant_id).await? {
        state.plans.active_tenant(db, location).await?
    } else {
        state.plans.get_active_for(Some(location)).await?
    };
    ok(plans)
}

#[utoipa::path(
    get,
    path = "/plans/{id}",
    params(
        ("id" = Uuid, Path, description = "Plan ID"),
    ),
    responses(
        (status = 200, description = "Get plan", body = PlanEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "plans"
)]
pub async fn get_plan(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Query(query): Query<PlanLocationQuery>,
    headers: HeaderMap,
) -> ApiResult<Plan> {
    let tenant_id = Uuid::parse_str(&claims.tenantId)
        .map_err(|_| AppError::Forbidden("Select an organization".into()))?;
    let requested = query.location_id.or(requested_location(&headers)?);
    if let Some(db) = state.tenant_db(tenant_id).await? {
        if claims.is_admin_or_staff() {
            let scope = LocationScope::resolve(&state.db, &claims, "plans:read", requested).await?;
            authorize_tenant_plan(db.clone(), id, &scope, false).await?;
            return ok(state.plans.get_tenant(db, id, requested).await?);
        }
        let location = pricing_location(&state, &claims, requested).await?;
        return ok(state.plans.get_tenant(db, id, Some(location)).await?);
    }
    if claims.is_admin_or_staff() {
        let scope = LocationScope::resolve(&state.db, &claims, "plans:read", None).await?;
        catalog_scope::get(&state.db, "plans", id, &scope, false).await?;
        if requested.is_none() {
            return ok(state.plans.get_by_id(id).await?);
        }
    }
    let location = pricing_location(&state, &claims, requested).await?;
    let plan = state.plans.get_by_id_for(id, Some(location)).await?;
    ok(plan)
}

#[utoipa::path(
    post,
    path = "/plans",
    request_body = CreatePlanDto,
    responses(
        (status = 201, description = "Create plan", body = PlanEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "plans"
)]
pub async fn create_plan(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<CatalogCreate<CreatePlanDto>>,
) -> ApiResult<Plan> {
    let scope = LocationScope::resolve(&state.db, &claims, "plans:write", None).await?;
    let requested = payload.location_ids.or(if scope.organization_admin {
        None
    } else {
        requested_location(&headers)?.map(|id| vec![id])
    });
    let locations = catalog_scope::create_locations(&state.db, &scope, requested).await?;
    let dto = payload.item;
    let plan = if let Some(db) = state.tenant_db(scope.organization_id).await? {
        state
            .plans
            .create_tenant(db, locations, dto, claims.user_id_uuid())
            .await?
    } else {
        crate::services::PlanService::new(
            state.db.clone(),
            state.cache.clone(),
            state.config.clone(),
        )
        .with_locations(locations)
        .create(dto, claims.user_id_uuid())
        .await?
    };
    created(plan)
}

#[utoipa::path(
    patch,
    path = "/plans/{id}",
    params(
        ("id" = Uuid, Path, description = "Plan ID"),
    ),
    request_body = UpdatePlanDto,
    responses(
        (status = 200, description = "Update plan", body = PlanEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "plans"
)]
pub async fn update_plan(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdatePlanDto>,
) -> ApiResult<Plan> {
    let scope = LocationScope::resolve(&state.db, &claims, "plans:write", None).await?;
    let plan = if let Some(db) = state.tenant_db(scope.organization_id).await? {
        authorize_tenant_plan(db.clone(), id, &scope, true).await?;
        state
            .plans
            .update_tenant(db, id, dto, claims.user_id_uuid())
            .await?
    } else {
        catalog_scope::get(&state.db, "plans", id, &scope, true).await?;
        state.plans.update(id, dto, claims.user_id_uuid()).await?
    };
    ok(plan)
}

#[utoipa::path(
    delete,
    path = "/plans/{id}",
    params(
        ("id" = Uuid, Path, description = "Plan ID"),
    ),
    responses(
        (status = 204, description = "Soft-deactivated (isActive=false)"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "plans"
)]
pub async fn delete_plan(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, crate::error::AppError> {
    let scope = LocationScope::resolve(&state.db, &claims, "plans:write", None).await?;
    if let Some(db) = state.tenant_db(scope.organization_id).await? {
        authorize_tenant_plan(db.clone(), id, &scope, true).await?;
        state.plans.delete_tenant(db, id).await?;
    } else {
        catalog_scope::get(&state.db, "plans", id, &scope, true).await?;
        state.plans.delete(id).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

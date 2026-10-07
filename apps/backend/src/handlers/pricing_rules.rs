use std::{collections::HashSet, sync::Arc};

use axum::extract::{Path, Query, State};
use axum::Json;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{ok, ApiResult};
use crate::error::AppError;
use crate::middleware::AuthUser;
use crate::models::{
    CreatePricingRuleSetDto, CreatePricingRuleVersionDto, PricingRuleSet, PricingRuleSetDraft,
    PricingRuleSetQuery, PricingRuleVersion, PricingSimulationDto, PricingSimulationResult,
    PublishPricingRuleVersionDto,
};

fn actor_id(claims: &crate::dto::JwtUserClaims) -> Result<Uuid, AppError> {
    claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user identity".to_string()))
}

async fn require(
    state: &AppState,
    claims: &crate::dto::JwtUserClaims,
    organization_id: Uuid,
    permission: &str,
) -> Result<Uuid, AppError> {
    let actor_id = actor_id(claims)?;
    crate::repositories::TenantSettingsRepository::new(state.business_db(&claims).await?)
        .ensure_access(organization_id, actor_id, permission)
        .await?;
    Ok(actor_id)
}

async fn require_rule_set(
    state: &AppState,
    claims: &crate::dto::JwtUserClaims,
    organization_id: Uuid,
    set_id: Uuid,
    permission: &str,
) -> Result<Uuid, AppError> {
    let actor_id = require(state, claims, organization_id, permission).await?;
    let rule_set = {
        let db = state.business_db(&claims).await?;
        state
            .pricing_rules
            .get_set_tenant(db, organization_id, set_id)
            .await?
    };
    if permission == "rules:read" {
        if !rule_set.location_ids.is_empty() {
            let scope = crate::access::scope::LocationScope::resolve_tenant(
                state.business_db(&claims).await?,
                claims,
                permission,
                None,
            )
            .await?;
            if !rule_set
                .location_ids
                .iter()
                .any(|id| scope.locations.contains(id))
            {
                return Err(AppError::Forbidden(
                    "This pricing policy is not available at your locations".into(),
                ));
            }
        }
    } else if rule_set.location_ids.is_empty() {
        crate::access::scope::require_tenant_admin(
            state.business_db(&claims).await?,
            organization_id,
            actor_id,
        )
        .await?;
    } else {
        for location in &rule_set.location_ids {
            crate::repositories::TenantSettingsRepository::new(state.business_db(&claims).await?)
                .ensure_location_permission(organization_id, *location, actor_id, permission)
                .await?;
        }
    }
    Ok(actor_id)
}

#[utoipa::path(
    get,
    path = "/organizations/{org_id}/pricing-rule-sets",
    params(("org_id" = Uuid, Path), PricingRuleSetQuery),
    responses((status = 200, body = crate::openapi::responses::PricingRuleSetListEnvelope)),
    security(("bearer_auth" = [])), tag = "pricing-rules"
)]
pub async fn list_rule_sets(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(org_id): Path<Uuid>,
    Query(query): Query<PricingRuleSetQuery>,
) -> ApiResult<Vec<PricingRuleSet>> {
    let actor_id = require(&state, &claims, org_id, "rules:read").await?;
    if let Some(location_id) = query.location_id {
        crate::repositories::TenantSettingsRepository::new(state.business_db(&claims).await?)
            .ensure_location_permission(org_id, location_id, actor_id, "rules:read")
            .await?;
    }
    let mut rule_sets = {
        let db = state.business_db(&claims).await?;
        state
            .pricing_rules
            .list_tenant(db, org_id, query.location_id)
            .await?
    };
    if query.location_id.is_none() {
        let assigned_locations: HashSet<Uuid> = state
            .config
            .list_locations(org_id, actor_id)
            .await?
            .into_iter()
            .map(|location| location.id)
            .collect();
        let mut allowed_locations = HashSet::new();
        for location_id in assigned_locations {
            if crate::repositories::TenantSettingsRepository::new(state.business_db(&claims).await?)
                .ensure_location_permission(org_id, location_id, actor_id, "rules:read")
                .await
                .is_ok()
            {
                allowed_locations.insert(location_id);
            }
        }
        rule_sets.retain(|rule_set| {
            rule_set.location_ids.is_empty()
                || rule_set
                    .location_ids
                    .iter()
                    .any(|id| allowed_locations.contains(id))
        });
    }
    ok(rule_sets)
}

#[utoipa::path(
    post,
    path = "/organizations/{org_id}/pricing-rule-sets",
    params(("org_id" = Uuid, Path)), request_body = CreatePricingRuleSetDto,
    responses((status = 200, body = crate::openapi::responses::PricingRuleSetDraftEnvelope)),
    security(("bearer_auth" = [])), tag = "pricing-rules"
)]
pub async fn create_rule_set(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(org_id): Path<Uuid>,
    Json(mut dto): Json<CreatePricingRuleSetDto>,
) -> ApiResult<PricingRuleSetDraft> {
    let actor_id = require(&state, &claims, org_id, "rules:edit").await?;
    if let Some(id) = dto.location_id {
        if !dto.location_ids.contains(&id) {
            dto.location_ids.push(id);
        }
    }
    dto.location_ids.sort();
    dto.location_ids.dedup();
    if dto.location_ids.is_empty() {
        crate::access::scope::require_tenant_admin(
            state.business_db(&claims).await?,
            org_id,
            actor_id,
        )
        .await?;
    }
    for id in &dto.location_ids {
        crate::repositories::TenantSettingsRepository::new(state.business_db(&claims).await?)
            .ensure_location_permission(org_id, *id, actor_id, "rules:edit")
            .await?;
    }
    let (rule_set, version) = {
        let db = state.business_db(&claims).await?;
        state
            .pricing_rules
            .create_tenant(db, org_id, dto, actor_id)
            .await?
    };
    ok(PricingRuleSetDraft { rule_set, version })
}

#[utoipa::path(
    get,
    path = "/organizations/{org_id}/pricing-rule-sets/{set_id}/versions",
    params(("org_id" = Uuid, Path), ("set_id" = Uuid, Path)),
    responses((status = 200, body = crate::openapi::responses::PricingRuleVersionListEnvelope)),
    security(("bearer_auth" = [])), tag = "pricing-rules"
)]
pub async fn list_versions(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path((org_id, set_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Vec<PricingRuleVersion>> {
    require_rule_set(&state, &claims, org_id, set_id, "rules:read").await?;
    {
        let db = state.business_db(&claims).await?;
        ok(state
            .pricing_rules
            .versions_tenant(db, org_id, set_id)
            .await?)
    }
}

#[utoipa::path(
    post,
    path = "/organizations/{org_id}/pricing-rule-sets/{set_id}/versions",
    params(("org_id" = Uuid, Path), ("set_id" = Uuid, Path)),
    request_body = CreatePricingRuleVersionDto,
    responses((status = 200, body = crate::openapi::responses::PricingRuleVersionEnvelope)),
    security(("bearer_auth" = [])), tag = "pricing-rules"
)]
pub async fn create_version(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path((org_id, set_id)): Path<(Uuid, Uuid)>,
    Json(dto): Json<CreatePricingRuleVersionDto>,
) -> ApiResult<PricingRuleVersion> {
    let actor_id = require_rule_set(&state, &claims, org_id, set_id, "rules:edit").await?;
    {
        let db = state.business_db(&claims).await?;
        ok(state
            .pricing_rules
            .create_version_tenant(db, org_id, set_id, dto, actor_id)
            .await?)
    }
}

#[utoipa::path(
    post,
    path = "/organizations/{org_id}/pricing-rule-sets/{set_id}/versions/{version_id}/validate",
    params(("org_id" = Uuid, Path), ("set_id" = Uuid, Path), ("version_id" = Uuid, Path)),
    responses((status = 200, body = crate::openapi::responses::PricingRuleVersionEnvelope)),
    security(("bearer_auth" = [])), tag = "pricing-rules"
)]
pub async fn validate_version(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path((org_id, set_id, version_id)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<PricingRuleVersion> {
    require_rule_set(&state, &claims, org_id, set_id, "rules:edit").await?;
    {
        let db = state.business_db(&claims).await?;
        ok(state
            .pricing_rules
            .validate_tenant(db, org_id, set_id, version_id)
            .await?)
    }
}

#[utoipa::path(
    post,
    path = "/organizations/{org_id}/pricing-rule-sets/{set_id}/versions/{version_id}/simulate",
    params(("org_id" = Uuid, Path), ("set_id" = Uuid, Path), ("version_id" = Uuid, Path)),
    request_body = PricingSimulationDto,
    responses((status = 200, body = crate::openapi::responses::PricingSimulationEnvelope)),
    security(("bearer_auth" = [])), tag = "pricing-rules"
)]
pub async fn simulate_version(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path((org_id, set_id, version_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(dto): Json<PricingSimulationDto>,
) -> ApiResult<PricingSimulationResult> {
    let actor_id = require_rule_set(&state, &claims, org_id, set_id, "rules:read").await?;
    if let Some(location_id) = dto.location_id {
        crate::repositories::TenantSettingsRepository::new(state.business_db(&claims).await?)
            .ensure_location_permission(org_id, location_id, actor_id, "rules:read")
            .await?;
    }
    let db = state.business_db(&claims).await?;
    let settings = state
        .config
        .effective_tenant(
            db.clone(),
            org_id,
            crate::models::EffectiveSettingsQuery {
                location_id: dto.location_id,
                category: None,
            },
        )
        .await?;
    let timezone = settings
        .iter()
        .find(|s| s.key == "venue.timezone")
        .and_then(|s| s.value.as_str())
        .ok_or_else(|| AppError::Internal("Tenant timezone is missing".into()))?;
    let currency = settings
        .iter()
        .find(|s| s.key == "pricing.currency")
        .and_then(|s| s.value.as_str())
        .unwrap_or("INR");
    ok(state
        .pricing_rules
        .simulate_tenant(db, org_id, set_id, version_id, dto, timezone, currency)
        .await?)
}

#[utoipa::path(
    post,
    path = "/organizations/{org_id}/pricing-rule-sets/{set_id}/versions/{version_id}/publish",
    params(("org_id" = Uuid, Path), ("set_id" = Uuid, Path), ("version_id" = Uuid, Path)),
    request_body = PublishPricingRuleVersionDto,
    responses((status = 200, body = crate::openapi::responses::PricingRuleVersionEnvelope)),
    security(("bearer_auth" = [])), tag = "pricing-rules"
)]
pub async fn publish_version(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path((org_id, set_id, version_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(dto): Json<PublishPricingRuleVersionDto>,
) -> ApiResult<PricingRuleVersion> {
    let actor_id = require_rule_set(&state, &claims, org_id, set_id, "rules:publish").await?;
    {
        let db = state.business_db(&claims).await?;
        ok(state
            .pricing_rules
            .publish_tenant(db, org_id, set_id, version_id, dto, actor_id)
            .await?)
    }
}

#[utoipa::path(
    post,
    path = "/organizations/{org_id}/pricing-rule-sets/{set_id}/versions/{version_id}/rollback",
    params(("org_id" = Uuid, Path), ("set_id" = Uuid, Path), ("version_id" = Uuid, Path)),
    responses((status = 200, body = crate::openapi::responses::PricingRuleVersionEnvelope)),
    security(("bearer_auth" = [])), tag = "pricing-rules"
)]
pub async fn rollback_version(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path((org_id, set_id, version_id)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<PricingRuleVersion> {
    let actor_id = require_rule_set(&state, &claims, org_id, set_id, "rules:publish").await?;
    {
        let db = state.business_db(&claims).await?;
        ok(state
            .pricing_rules
            .rollback_tenant(db, org_id, set_id, version_id, actor_id)
            .await?)
    }
}

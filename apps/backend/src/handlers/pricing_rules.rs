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
    state
        .config
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
    let rule_set = if let Some(db) = state.tenant_db(organization_id).await? {
        state
            .pricing_rules
            .get_set_tenant(db, organization_id, set_id)
            .await?
    } else {
        state.pricing_rules.get_set(organization_id, set_id).await?
    };
    if permission == "rules:read" {
        if !rule_set.location_ids.is_empty() {
            let scope =
                crate::access::scope::LocationScope::resolve(&state.db, claims, permission, None)
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
        crate::access::scope::require_organization_admin(&state.db, organization_id, actor_id)
            .await?;
    } else {
        for location in &rule_set.location_ids {
            state
                .config
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
        state
            .config
            .ensure_location_permission(org_id, location_id, actor_id, "rules:read")
            .await?;
    }
    let mut rule_sets = if let Some(db) = state.tenant_db(org_id).await? {
        state
            .pricing_rules
            .list_tenant(db, org_id, query.location_id)
            .await?
    } else {
        state.pricing_rules.list(org_id, query.location_id).await?
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
            if state
                .config
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
        crate::access::scope::require_organization_admin(&state.db, org_id, actor_id).await?;
    }
    for id in &dto.location_ids {
        state
            .config
            .ensure_location_permission(org_id, *id, actor_id, "rules:edit")
            .await?;
    }
    let (rule_set, version) = if let Some(db) = state.tenant_db(org_id).await? {
        state
            .pricing_rules
            .create_tenant(db, org_id, dto, actor_id)
            .await?
    } else {
        state.pricing_rules.create(org_id, dto, actor_id).await?
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
    if let Some(db) = state.tenant_db(org_id).await? {
        ok(state
            .pricing_rules
            .versions_tenant(db, org_id, set_id)
            .await?)
    } else {
        ok(state.pricing_rules.versions(org_id, set_id).await?)
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
    if let Some(db) = state.tenant_db(org_id).await? {
        ok(state
            .pricing_rules
            .create_version_tenant(db, org_id, set_id, dto, actor_id)
            .await?)
    } else {
        ok(state
            .pricing_rules
            .create_version(org_id, set_id, dto, actor_id)
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
    if let Some(db) = state.tenant_db(org_id).await? {
        ok(state
            .pricing_rules
            .validate_tenant(db, org_id, set_id, version_id)
            .await?)
    } else {
        ok(state
            .pricing_rules
            .validate(org_id, set_id, version_id)
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
        state
            .config
            .ensure_location_permission(org_id, location_id, actor_id, "rules:read")
            .await?;
    }
    let timezone = state
        .config
        .resolve_value(org_id, dto.location_id, "venue.timezone")
        .await?
        .as_str()
        .unwrap_or(&state.settings.cafe_timezone)
        .to_string();
    let tenant_db = state.tenant_db(org_id).await?;
    let currency_value = if let Some(db) = tenant_db.clone() {
        state
            .config
            .resolve_pricing_value_tenant(db, org_id, dto.location_id, "pricing.currency")
            .await?
    } else {
        state
            .config
            .resolve_value(org_id, dto.location_id, "pricing.currency")
            .await?
    };
    let currency = currency_value.as_str().unwrap_or("INR").to_string();
    if let Some(db) = tenant_db {
        ok(state
            .pricing_rules
            .simulate_tenant(db, org_id, set_id, version_id, dto, &timezone, &currency)
            .await?)
    } else {
        ok(state
            .pricing_rules
            .simulate(org_id, set_id, version_id, dto, &timezone, &currency)
            .await?)
    }
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
    if let Some(db) = state.tenant_db(org_id).await? {
        ok(state
            .pricing_rules
            .publish_tenant(db, org_id, set_id, version_id, dto, actor_id)
            .await?)
    } else {
        ok(state
            .pricing_rules
            .publish(org_id, set_id, version_id, dto, actor_id)
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
    if let Some(db) = state.tenant_db(org_id).await? {
        ok(state
            .pricing_rules
            .rollback_tenant(db, org_id, set_id, version_id, actor_id)
            .await?)
    } else {
        ok(state
            .pricing_rules
            .rollback(org_id, set_id, version_id, actor_id)
            .await?)
    }
}

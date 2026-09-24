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
    let rule_set = state.pricing_rules.get_set(organization_id, set_id).await?;
    if let Some(location_id) = rule_set.location_id {
        state
            .config
            .ensure_location_access(organization_id, location_id, actor_id)
            .await?;
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
            .ensure_location_access(org_id, location_id, actor_id)
            .await?;
    }
    let mut rule_sets = state.pricing_rules.list(org_id, query.location_id).await?;
    if query.location_id.is_none() {
        let allowed_locations: HashSet<Uuid> = state
            .config
            .list_locations(org_id, actor_id)
            .await?
            .into_iter()
            .map(|location| location.id)
            .collect();
        rule_sets.retain(|rule_set| {
            rule_set
                .location_id
                .is_none_or(|location_id| allowed_locations.contains(&location_id))
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
    Json(dto): Json<CreatePricingRuleSetDto>,
) -> ApiResult<PricingRuleSetDraft> {
    let actor_id = require(&state, &claims, org_id, "rules:edit").await?;
    if let Some(location_id) = dto.location_id {
        state
            .config
            .ensure_location_access(org_id, location_id, actor_id)
            .await?;
    }
    let (rule_set, version) = state.pricing_rules.create(org_id, dto, actor_id).await?;
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
    ok(state.pricing_rules.versions(org_id, set_id).await?)
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
    ok(state
        .pricing_rules
        .create_version(org_id, set_id, dto, actor_id)
        .await?)
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
    ok(state
        .pricing_rules
        .validate(org_id, set_id, version_id)
        .await?)
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
            .ensure_location_access(org_id, location_id, actor_id)
            .await?;
    }
    let timezone = state
        .config
        .resolve_value(org_id, dto.location_id, "venue.timezone")
        .await?
        .as_str()
        .unwrap_or(&state.settings.cafe_timezone)
        .to_string();
    let currency = state
        .config
        .resolve_value(org_id, dto.location_id, "pricing.currency")
        .await?
        .as_str()
        .unwrap_or("INR")
        .to_string();
    ok(state
        .pricing_rules
        .simulate(org_id, set_id, version_id, dto, &timezone, &currency)
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
    ok(state
        .pricing_rules
        .publish(org_id, set_id, version_id, dto, actor_id)
        .await?)
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
    ok(state
        .pricing_rules
        .rollback(org_id, set_id, version_id, actor_id)
        .await?)
}

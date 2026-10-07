use std::str::FromStr;
use std::sync::Arc;

use chrono::{Datelike, Utc};
use chrono_tz::Tz;
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    CreatePricingRuleSetDto, CreatePricingRuleVersionDto, PricingAction, PricingPolicy,
    PricingRule, PricingRuleSet, PricingRuleVersion, PricingSimulationDto, PricingSimulationResult,
    PricingTarget, PricingTraceStep, PublishPricingRuleVersionDto,
};
use crate::realtime::OutboxService;
use crate::repositories::{PricingPolicyRepository, TenantPricingPolicyRepository};
use crate::tenancy::TenantDb;

/// Values of the `products_category_enum` database type.
const PRODUCT_CATEGORIES: [&str; 4] = ["beverage", "snack", "meal", "other"];

#[derive(Clone)]
pub struct PricingPolicyService {
    repo: PricingPolicyRepository,
}

impl PricingPolicyService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: PricingPolicyRepository::new(pool),
        }
    }

    pub fn with_outbox(mut self, outbox: OutboxService) -> Self {
        self.repo = self.repo.with_outbox(outbox);
        self
    }

    pub async fn list(
        &self,
        organization_id: Uuid,
        location_id: Option<Uuid>,
    ) -> Result<Vec<PricingRuleSet>, AppError> {
        self.repo.list_sets(organization_id, location_id).await
    }

    pub async fn get_set(
        &self,
        organization_id: Uuid,
        set_id: Uuid,
    ) -> Result<PricingRuleSet, AppError> {
        self.repo.get_set(organization_id, set_id).await
    }

    pub async fn create(
        &self,
        organization_id: Uuid,
        mut dto: CreatePricingRuleSetDto,
        actor_id: Uuid,
    ) -> Result<(PricingRuleSet, PricingRuleVersion), AppError> {
        if let Some(id) = dto.location_id {
            if !dto.location_ids.contains(&id) {
                dto.location_ids.push(id);
            }
        }
        if dto.name.trim().len() < 3 {
            return Err(AppError::BadRequest(
                "Pricing rule set name must contain at least 3 characters".to_string(),
            ));
        }
        Self::validate_policy(&dto.policy)?;
        let policy = serde_json::to_value(&dto.policy)
            .map_err(|error| AppError::BadRequest(format!("Invalid pricing policy: {error}")))?;
        self.repo
            .create_set(
                organization_id,
                dto.location_ids.first().copied().or(dto.location_id),
                &dto.location_ids,
                dto.name.trim(),
                dto.description.as_deref(),
                &policy,
                actor_id,
            )
            .await
    }

    pub async fn create_version(
        &self,
        organization_id: Uuid,
        set_id: Uuid,
        dto: CreatePricingRuleVersionDto,
        actor_id: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        Self::validate_policy(&dto.policy)?;
        let policy = serde_json::to_value(dto.policy)
            .map_err(|error| AppError::BadRequest(format!("Invalid pricing policy: {error}")))?;
        self.repo
            .create_version(organization_id, set_id, &policy, actor_id)
            .await
    }

    pub async fn versions(
        &self,
        organization_id: Uuid,
        set_id: Uuid,
    ) -> Result<Vec<PricingRuleVersion>, AppError> {
        self.repo.versions(organization_id, set_id).await
    }

    pub async fn validate(
        &self,
        organization_id: Uuid,
        set_id: Uuid,
        version_id: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        let version = self
            .repo
            .get_version(organization_id, set_id, version_id)
            .await?;
        let policy: PricingPolicy = serde_json::from_value(version.policy.clone())
            .map_err(|error| AppError::BadRequest(format!("Invalid pricing policy: {error}")))?;
        Self::validate_policy(&policy)?;
        self.repo
            .mark_validated(organization_id, set_id, version_id)
            .await
    }

    pub async fn simulate(
        &self,
        organization_id: Uuid,
        set_id: Uuid,
        version_id: Uuid,
        input: PricingSimulationDto,
        timezone: &str,
        currency: &str,
    ) -> Result<PricingSimulationResult, AppError> {
        let rule_set = self.repo.get_set(organization_id, set_id).await?;
        if let Some(requested) = input.location_id {
            if !rule_set.location_ids.is_empty() && !rule_set.location_ids.contains(&requested) {
                return Err(AppError::BadRequest(
                    "Pricing rule set is not available for the requested location".to_string(),
                ));
            }
        }
        let version = self
            .repo
            .get_version(organization_id, set_id, version_id)
            .await?;
        let policy: PricingPolicy = serde_json::from_value(version.policy.clone())
            .map_err(|error| AppError::BadRequest(format!("Invalid pricing policy: {error}")))?;
        Self::validate_policy(&policy)?;
        let result = Self::evaluate(&policy, &input, timezone, currency)?;
        self.repo
            .record_simulation(organization_id, set_id, version_id, &result.simulation_hash)
            .await?;
        Ok(result)
    }

    pub async fn publish(
        &self,
        organization_id: Uuid,
        set_id: Uuid,
        version_id: Uuid,
        dto: PublishPricingRuleVersionDto,
        actor_id: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        let effective_at = dto.effective_at.unwrap_or_else(Utc::now);
        let version = self
            .repo
            .publish(organization_id, set_id, version_id, effective_at, actor_id)
            .await?;
        Ok(version)
    }

    pub async fn rollback(
        &self,
        organization_id: Uuid,
        set_id: Uuid,
        target_version_id: Uuid,
        actor_id: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        let target = self
            .repo
            .get_version(organization_id, set_id, target_version_id)
            .await?;
        let policy: PricingPolicy = serde_json::from_value(target.policy.clone())
            .map_err(|error| AppError::BadRequest(format!("Invalid pricing policy: {error}")))?;
        Self::validate_policy(&policy)?;
        let draft = self
            .repo
            .create_version(organization_id, set_id, &target.policy, actor_id)
            .await?;
        self.repo
            .mark_validated(organization_id, set_id, draft.id)
            .await?;
        let previous_hash = target.simulation_hash.ok_or_else(|| {
            AppError::Conflict("The target version has no successful simulation".to_string())
        })?;
        self.repo
            .record_simulation(organization_id, set_id, draft.id, &previous_hash)
            .await?;
        let published = self
            .repo
            .publish(organization_id, set_id, draft.id, Utc::now(), actor_id)
            .await?;
        Ok(published)
    }

    pub async fn activate_due(&self) -> Result<u64, AppError> {
        let activated = self.repo.activate_due().await?;
        Ok(activated.len() as u64)
    }

    pub async fn list_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        location_id: Option<Uuid>,
    ) -> Result<Vec<PricingRuleSet>, AppError> {
        TenantPricingPolicyRepository::new(db)
            .list_sets(organization_id, location_id)
            .await
    }

    pub async fn get_set_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        set_id: Uuid,
    ) -> Result<PricingRuleSet, AppError> {
        TenantPricingPolicyRepository::new(db)
            .get_set(organization_id, set_id)
            .await
    }

    pub async fn create_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        mut dto: CreatePricingRuleSetDto,
        actor_id: Uuid,
    ) -> Result<(PricingRuleSet, PricingRuleVersion), AppError> {
        if let Some(id) = dto.location_id {
            if !dto.location_ids.contains(&id) {
                dto.location_ids.push(id);
            }
        }
        if dto.name.trim().len() < 3 {
            return Err(AppError::BadRequest(
                "Pricing rule set name must contain at least 3 characters".into(),
            ));
        }
        Self::validate_policy(&dto.policy)?;
        let policy = serde_json::to_value(&dto.policy)
            .map_err(|error| AppError::BadRequest(format!("Invalid pricing policy: {error}")))?;
        TenantPricingPolicyRepository::new(db)
            .create_set(
                organization_id,
                dto.location_ids.first().copied().or(dto.location_id),
                &dto.location_ids,
                dto.name.trim(),
                dto.description.as_deref(),
                &policy,
                actor_id,
            )
            .await
    }

    pub async fn create_version_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        set_id: Uuid,
        dto: CreatePricingRuleVersionDto,
        actor_id: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        Self::validate_policy(&dto.policy)?;
        let policy = serde_json::to_value(dto.policy)
            .map_err(|error| AppError::BadRequest(format!("Invalid pricing policy: {error}")))?;
        TenantPricingPolicyRepository::new(db)
            .create_version(organization_id, set_id, &policy, actor_id)
            .await
    }

    pub async fn versions_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        set_id: Uuid,
    ) -> Result<Vec<PricingRuleVersion>, AppError> {
        TenantPricingPolicyRepository::new(db)
            .versions(organization_id, set_id)
            .await
    }

    pub async fn validate_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        set_id: Uuid,
        version_id: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        let repo = TenantPricingPolicyRepository::new(db);
        let version = repo
            .get_version(organization_id, set_id, version_id)
            .await?;
        let policy: PricingPolicy = serde_json::from_value(version.policy)
            .map_err(|error| AppError::BadRequest(format!("Invalid pricing policy: {error}")))?;
        Self::validate_policy(&policy)?;
        repo.mark_validated(organization_id, set_id, version_id)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn simulate_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        set_id: Uuid,
        version_id: Uuid,
        input: PricingSimulationDto,
        timezone: &str,
        currency: &str,
    ) -> Result<PricingSimulationResult, AppError> {
        let repo = TenantPricingPolicyRepository::new(db);
        let rule_set = repo.get_set(organization_id, set_id).await?;
        if let Some(requested) = input.location_id {
            if !rule_set.location_ids.is_empty() && !rule_set.location_ids.contains(&requested) {
                return Err(AppError::BadRequest(
                    "Pricing rule set is not available for the requested location".into(),
                ));
            }
        }
        let version = repo
            .get_version(organization_id, set_id, version_id)
            .await?;
        let policy: PricingPolicy = serde_json::from_value(version.policy)
            .map_err(|error| AppError::BadRequest(format!("Invalid pricing policy: {error}")))?;
        Self::validate_policy(&policy)?;
        let result = Self::evaluate(&policy, &input, timezone, currency)?;
        repo.record_simulation(organization_id, set_id, version_id, &result.simulation_hash)
            .await?;
        Ok(result)
    }

    pub async fn publish_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        set_id: Uuid,
        version_id: Uuid,
        dto: PublishPricingRuleVersionDto,
        actor_id: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        TenantPricingPolicyRepository::new(db)
            .publish(
                organization_id,
                set_id,
                version_id,
                dto.effective_at.unwrap_or_else(Utc::now),
                actor_id,
            )
            .await
    }

    pub async fn rollback_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        set_id: Uuid,
        target_version_id: Uuid,
        actor_id: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        let repo = TenantPricingPolicyRepository::new(db);
        let target = repo
            .get_version(organization_id, set_id, target_version_id)
            .await?;
        let policy: PricingPolicy = serde_json::from_value(target.policy.clone())
            .map_err(|error| AppError::BadRequest(format!("Invalid pricing policy: {error}")))?;
        Self::validate_policy(&policy)?;
        let draft = repo
            .create_version(organization_id, set_id, &target.policy, actor_id)
            .await?;
        repo.mark_validated(organization_id, set_id, draft.id)
            .await?;
        let hash = target.simulation_hash.ok_or_else(|| {
            AppError::Conflict("The target version has no successful simulation".into())
        })?;
        repo.record_simulation(organization_id, set_id, draft.id, &hash)
            .await?;
        repo.publish(organization_id, set_id, draft.id, Utc::now(), actor_id)
            .await
    }

    pub fn validate_policy(policy: &PricingPolicy) -> Result<(), AppError> {
        let base = decimal("baseRate", &policy.base_rate)?;
        if base < Decimal::ZERO {
            return Err(AppError::BadRequest(
                "baseRate cannot be negative".to_string(),
            ));
        }
        if policy.rounding_scale > 4 {
            return Err(AppError::BadRequest(
                "roundingScale cannot exceed 4".to_string(),
            ));
        }
        let minimum = policy
            .minimum_price
            .as_deref()
            .map(|value| decimal("minimumPrice", value))
            .transpose()?;
        let maximum = policy
            .maximum_price
            .as_deref()
            .map(|value| decimal("maximumPrice", value))
            .transpose()?;
        if minimum.zip(maximum).is_some_and(|(min, max)| min > max) {
            return Err(AppError::BadRequest(
                "minimumPrice cannot exceed maximumPrice".to_string(),
            ));
        }

        let mut ids = std::collections::HashSet::new();
        for rule in &policy.rules {
            if rule.id.trim().is_empty() || !ids.insert(rule.id.clone()) {
                return Err(AppError::BadRequest(
                    "Pricing rule IDs must be non-empty and unique".to_string(),
                ));
            }
            if rule.name.trim().is_empty() {
                return Err(AppError::BadRequest(format!(
                    "Pricing rule '{}' requires a name",
                    rule.id
                )));
            }
            if rule.weekdays.iter().any(|day| !(1..=7).contains(day)) {
                return Err(AppError::BadRequest(format!(
                    "Pricing rule '{}' weekdays must be 1 (Monday) through 7 (Sunday)",
                    rule.id
                )));
            }
            match rule.target {
                PricingTarget::Products if !rule.device_types.is_empty() => {
                    return Err(AppError::BadRequest(format!(
                        "Pricing rule '{}' targets products and cannot filter by device type",
                        rule.id
                    )));
                }
                PricingTarget::Sessions | PricingTarget::Deduction
                    if !rule.product_ids.is_empty() || !rule.categories.is_empty() =>
                {
                    return Err(AppError::BadRequest(format!(
                        "Pricing rule '{}' must target products to filter by product or category",
                        rule.id
                    )));
                }
                _ => {}
            }
            if let Some(category) = rule
                .categories
                .iter()
                .find(|item| !PRODUCT_CATEGORIES.contains(&item.as_str()))
            {
                return Err(AppError::BadRequest(format!(
                    "Pricing rule '{}' has unknown product category '{category}'",
                    rule.id
                )));
            }
            if rule.start_time.is_some() != rule.end_time.is_some() {
                return Err(AppError::BadRequest(format!(
                    "Pricing rule '{}' must provide both startTime and endTime",
                    rule.id
                )));
            }
            if rule
                .starts_at
                .zip(rule.ends_at)
                .is_some_and(|(start, end)| start >= end)
            {
                return Err(AppError::BadRequest(format!(
                    "Pricing rule '{}' startsAt must precede endsAt",
                    rule.id
                )));
            }
            if rule.target == PricingTarget::Deduction {
                let (PricingAction::Fixed { value } | PricingAction::Multiplier { value }) =
                    &rule.action;
                let speed = decimal("deduction speed", value)?;
                if speed <= Decimal::ZERO || speed > Decimal::from(100) {
                    return Err(AppError::BadRequest(
                        "Deduction speed must be greater than 0 and at most 100".to_string(),
                    ));
                }
            }
            match &rule.action {
                PricingAction::Fixed { value }
                    if decimal("fixed value", value)? < Decimal::ZERO =>
                {
                    return Err(AppError::BadRequest(format!(
                        "Pricing rule '{}' fixed value cannot be negative",
                        rule.id
                    )));
                }
                PricingAction::Multiplier { value }
                    if decimal("multiplier", value)? <= Decimal::ZERO =>
                {
                    return Err(AppError::BadRequest(format!(
                        "Pricing rule '{}' multiplier must be positive",
                        rule.id
                    )));
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn evaluate(
        policy: &PricingPolicy,
        input: &PricingSimulationDto,
        timezone: &str,
        currency: &str,
    ) -> Result<PricingSimulationResult, AppError> {
        let tz: Tz = timezone
            .parse()
            .map_err(|_| AppError::BadRequest(format!("Invalid IANA timezone '{timezone}'")))?;
        let local = input.at.with_timezone(&tz);
        let target =
            input
                .target
                .unwrap_or(if input.product_id.is_some() || input.category.is_some() {
                    PricingTarget::Products
                } else {
                    PricingTarget::Sessions
                });
        let base = if target == PricingTarget::Deduction {
            Decimal::ONE
        } else {
            input
                .base_rate
                .as_deref()
                .map(|value| decimal("baseRate", value))
                .transpose()?
                .unwrap_or(decimal("baseRate", &policy.base_rate)?)
        };
        let mut current = base;
        let mut trace = Vec::new();
        let product = (input.product_id.is_some() || input.category.is_some()).then(|| {
            (
                input.product_id,
                input.category.as_deref().unwrap_or_default(),
            )
        });
        let mut rules: Vec<&PricingRule> = policy
            .rules
            .iter()
            .filter(|rule| {
                rule.target == target
                    && Self::matches(
                        rule,
                        input.device_type.as_deref(),
                        product,
                        input.at,
                        local.time(),
                        local.weekday().number_from_monday() as u8,
                    )
            })
            .collect();
        rules.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.id.cmp(&right.id))
        });
        for rule in rules {
            let before = current;
            let action = match &rule.action {
                PricingAction::Fixed { value } => {
                    current = decimal("fixed value", value)?;
                    format!("fixed {value}")
                }
                PricingAction::Multiplier { value } => {
                    current *= decimal("multiplier", value)?;
                    format!("multiplier {value}")
                }
            };
            trace.push(PricingTraceStep {
                rule_id: rule.id.clone(),
                rule_name: rule.name.clone(),
                action,
                before: before.normalize().to_string(),
                after: current.normalize().to_string(),
            });
        }
        if target != PricingTarget::Deduction {
            if let Some(minimum) = policy.minimum_price.as_deref() {
                current = current.max(decimal("minimumPrice", minimum)?);
            }
            if let Some(maximum) = policy.maximum_price.as_deref() {
                current = current.min(decimal("maximumPrice", maximum)?);
            }
        }
        current = current.round_dp(if target == PricingTarget::Deduction {
            4
        } else {
            policy.rounding_scale
        });

        let hash_input = serde_json::json!({
            "policy": policy,
            "input": input,
            "timezone": timezone,
            "currency": currency,
            "result": current.normalize().to_string(),
        });
        let bytes = serde_json::to_vec(&hash_input)
            .map_err(|error| AppError::Internal(format!("Simulation hashing failed: {error}")))?;
        let simulation_hash = hex::encode(Sha256::digest(bytes));
        Ok(PricingSimulationResult {
            base_rate: base.normalize().to_string(),
            final_price: current.normalize().to_string(),
            currency: if target == PricingTarget::Deduction {
                "credits/min".to_string()
            } else {
                currency.to_string()
            },
            timezone: timezone.to_string(),
            trace,
            simulation_hash,
        })
    }

    /// Compatibility evaluator for the existing product day/night columns.
    /// This keeps the legacy numeric API while routing time-window behavior
    /// through the same deterministic, timezone-aware pricing evaluator.
    pub fn evaluate_legacy_product_price(
        day_price: f64,
        night_price: f64,
        at: chrono::DateTime<Utc>,
        timezone: &str,
        night_start: &str,
        night_end: &str,
    ) -> Result<f64, AppError> {
        let start_time = chrono::NaiveTime::parse_from_str(night_start, "%H:%M")
            .map_err(|_| AppError::BadRequest("Invalid night pricing start time".to_string()))?;
        let end_time = chrono::NaiveTime::parse_from_str(night_end, "%H:%M")
            .map_err(|_| AppError::BadRequest("Invalid night pricing end time".to_string()))?;
        let policy = PricingPolicy {
            base_rate: day_price.to_string(),
            rounding_scale: 2,
            minimum_price: Some("0".to_string()),
            maximum_price: None,
            rules: vec![PricingRule {
                id: "legacy-product-night-price".to_string(),
                name: "Legacy product night price".to_string(),
                priority: 100,
                target: PricingTarget::Sessions,
                device_types: vec![],
                product_ids: vec![],
                categories: vec![],
                weekdays: vec![],
                start_time: Some(start_time),
                end_time: Some(end_time),
                starts_at: None,
                ends_at: None,
                action: PricingAction::Fixed {
                    value: night_price.to_string(),
                },
            }],
        };
        let result = Self::evaluate(
            &policy,
            &PricingSimulationDto {
                target: None,
                location_id: None,
                device_type: None,
                at,
                base_rate: None,
                product_id: None,
                category: None,
            },
            timezone,
            "LEGACY",
        )?;
        result.final_price.parse::<f64>().map_err(|error| {
            AppError::Internal(format!("Legacy product price conversion failed: {error}"))
        })
    }

    /// Product rules from every published rule set that covers the venue:
    /// organization-wide sets plus sets scoped to `location_id`.
    pub async fn active_product_rules(
        &self,
        organization_id: Uuid,
        location_id: Option<Uuid>,
    ) -> Result<Vec<PricingRule>, AppError> {
        self.active_rules(organization_id, location_id, PricingTarget::Products)
            .await
    }

    pub async fn active_rules(
        &self,
        organization_id: Uuid,
        location_id: Option<Uuid>,
        target: PricingTarget,
    ) -> Result<Vec<PricingRule>, AppError> {
        let mut rules = Vec::new();
        for value in self
            .repo
            .active_policies(organization_id, location_id)
            .await?
        {
            let policy: PricingPolicy = serde_json::from_value(value).map_err(|error| {
                AppError::Internal(format!("Published pricing policy is invalid: {error}"))
            })?;
            Self::validate_policy(&policy)?;
            rules.extend(
                policy
                    .rules
                    .into_iter()
                    .filter(|rule| rule.target == target),
            );
        }
        Ok(rules)
    }

    pub async fn active_rules_tenant(
        &self,
        db: Arc<TenantDb>,
        organization_id: Uuid,
        location_id: Option<Uuid>,
        target: PricingTarget,
    ) -> Result<Vec<PricingRule>, AppError> {
        let mut rules = Vec::new();
        for value in TenantPricingPolicyRepository::new(db)
            .active_policies(organization_id, location_id)
            .await?
        {
            let policy: PricingPolicy = serde_json::from_value(value).map_err(|error| {
                AppError::Internal(format!("Published pricing policy is invalid: {error}"))
            })?;
            Self::validate_policy(&policy)?;
            rules.extend(
                policy
                    .rules
                    .into_iter()
                    .filter(|rule| rule.target == target),
            );
        }
        Ok(rules)
    }

    /// Combine published plan rules in the same priority order as the preview.
    /// Limits from applicable rule sets form a shared price range.
    pub async fn active_plan_policy(&self) -> Result<PricingPolicy, AppError> {
        self.active_plan_policy_for(
            crate::models::DEFAULT_ORGANIZATION_ID,
            Some(crate::models::DEFAULT_VENUE_LOCATION_ID),
        )
        .await
    }

    pub async fn active_plan_policy_for(
        &self,
        organization_id: Uuid,
        location_id: Option<Uuid>,
    ) -> Result<PricingPolicy, AppError> {
        let mut combined = PricingPolicy {
            base_rate: "0".into(),
            rules: vec![],
            rounding_scale: 2,
            minimum_price: None,
            maximum_price: None,
        };
        let mut has_plan_policy = false;
        for value in self
            .repo
            .active_policies(organization_id, location_id)
            .await?
        {
            let policy: PricingPolicy = serde_json::from_value(value).map_err(|error| {
                AppError::Internal(format!("Invalid published policy: {error}"))
            })?;
            Self::validate_policy(&policy)?;
            if !policy.rules.is_empty()
                && !policy
                    .rules
                    .iter()
                    .any(|rule| rule.target == PricingTarget::Sessions)
            {
                continue;
            }
            combined.rounding_scale = if has_plan_policy {
                combined.rounding_scale.min(policy.rounding_scale)
            } else {
                policy.rounding_scale
            };
            has_plan_policy = true;
            if let Some(value) = policy.minimum_price {
                let existing = combined
                    .minimum_price
                    .as_deref()
                    .map(|v| decimal("minimumPrice", v))
                    .transpose()?
                    .unwrap_or(Decimal::ZERO);
                combined.minimum_price =
                    Some(existing.max(decimal("minimumPrice", &value)?).to_string());
            }
            if let Some(value) = policy.maximum_price {
                let existing = combined
                    .maximum_price
                    .as_deref()
                    .map(|v| decimal("maximumPrice", v))
                    .transpose()?
                    .unwrap_or(Decimal::MAX);
                combined.maximum_price =
                    Some(existing.min(decimal("maximumPrice", &value)?).to_string());
            }
            combined.rules.extend(
                policy
                    .rules
                    .into_iter()
                    .filter(|r| r.target == PricingTarget::Sessions),
            );
        }
        if combined
            .minimum_price
            .as_deref()
            .zip(combined.maximum_price.as_deref())
            .is_some_and(|(min, max)| decimal("min", min).unwrap() > decimal("max", max).unwrap())
        {
            return Err(AppError::Conflict(
                "Published plan price limits conflict. Review the pricing policies.".into(),
            ));
        }
        Ok(combined)
    }

    /// Price rules adjust the selected plan's purchase price, never its minute balance.
    pub fn evaluate_plan_price(
        base: f64,
        device_type: Option<&str>,
        policy: &PricingPolicy,
        at: chrono::DateTime<Utc>,
        timezone: &str,
    ) -> Result<f64, AppError> {
        let input = PricingSimulationDto {
            target: Some(PricingTarget::Sessions),
            location_id: None,
            device_type: device_type.map(str::to_string),
            at,
            base_rate: Some(base.to_string()),
            product_id: None,
            category: None,
        };
        let result = Self::evaluate(policy, &input, timezone, "INR")?;
        result
            .final_price
            .parse()
            .map_err(|error| AppError::Internal(format!("Invalid plan price: {error}")))
    }

    /// The product's day/night price, adjusted by published product rules.
    /// Session settings such as base rate and min/max price do not apply to products.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_product_price(
        day_price: f64,
        night_price: f64,
        product_id: Uuid,
        category: &str,
        rules: &[PricingRule],
        at: chrono::DateTime<Utc>,
        timezone: &str,
        night_start: &str,
        night_end: &str,
    ) -> Result<f64, AppError> {
        let base = Self::evaluate_legacy_product_price(
            day_price,
            night_price,
            at,
            timezone,
            night_start,
            night_end,
        )?;
        if rules.is_empty() {
            return Ok(base);
        }
        let policy = PricingPolicy {
            base_rate: base.to_string(),
            rules: rules.to_vec(),
            rounding_scale: 2,
            minimum_price: Some("0".to_string()),
            maximum_price: None,
        };
        let result = Self::evaluate(
            &policy,
            &PricingSimulationDto {
                target: None,
                location_id: None,
                device_type: None,
                at,
                base_rate: None,
                product_id: Some(product_id),
                category: Some(category.to_string()),
            },
            timezone,
            "PRODUCT",
        )?;
        result.final_price.parse::<f64>().map_err(|error| {
            AppError::Internal(format!("Product price conversion failed: {error}"))
        })
    }

    pub(crate) fn matches(
        rule: &PricingRule,
        device_type: Option<&str>,
        product: Option<(Option<Uuid>, &str)>,
        at: chrono::DateTime<Utc>,
        local_time: chrono::NaiveTime,
        weekday: u8,
    ) -> bool {
        match (rule.target, product) {
            (PricingTarget::Sessions | PricingTarget::Deduction, Some(_))
            | (PricingTarget::Products, None) => return false,
            (PricingTarget::Products, Some((product_id, category))) => {
                let scoped = !rule.product_ids.is_empty() || !rule.categories.is_empty();
                let listed = product_id.is_some_and(|id| rule.product_ids.contains(&id))
                    || rule
                        .categories
                        .iter()
                        .any(|item| item.eq_ignore_ascii_case(category));
                if scoped && !listed {
                    return false;
                }
            }
            (PricingTarget::Sessions | PricingTarget::Deduction, None) => {}
        }
        if !rule.device_types.is_empty()
            && !device_type.is_some_and(|value| {
                rule.device_types
                    .iter()
                    .any(|item| item.eq_ignore_ascii_case(value))
            })
        {
            return false;
        }
        if !rule.weekdays.is_empty() && !rule.weekdays.contains(&weekday) {
            return false;
        }
        if rule.starts_at.is_some_and(|start| at < start)
            || rule.ends_at.is_some_and(|end| at >= end)
        {
            return false;
        }
        match (rule.start_time, rule.end_time) {
            (Some(start), Some(end)) if start <= end => local_time >= start && local_time < end,
            (Some(start), Some(end)) => local_time >= start || local_time < end,
            _ => true,
        }
    }
}

fn decimal(field: &str, value: &str) -> Result<Decimal, AppError> {
    Decimal::from_str(value)
        .map_err(|_| AppError::BadRequest(format!("{field} must be a decimal string")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveTime, TimeZone};

    fn policy() -> PricingPolicy {
        PricingPolicy {
            base_rate: "100".to_string(),
            rounding_scale: 2,
            minimum_price: None,
            maximum_price: None,
            rules: vec![PricingRule {
                id: "night".to_string(),
                name: "Night multiplier".to_string(),
                priority: 10,
                target: PricingTarget::Sessions,
                device_types: vec!["PS5".to_string()],
                product_ids: vec![],
                categories: vec![],
                weekdays: vec![],
                start_time: Some(NaiveTime::from_hms_opt(23, 0, 0).unwrap()),
                end_time: Some(NaiveTime::from_hms_opt(8, 0, 0).unwrap()),
                starts_at: None,
                ends_at: None,
                action: PricingAction::Multiplier {
                    value: "1.25".to_string(),
                },
            }],
        }
    }

    #[test]
    fn price_and_deduction_choices_are_independent() {
        let at = Utc.with_ymd_and_hms(2026, 9, 24, 18, 30, 0).unwrap();
        let mut p = policy();
        p.rules[0].device_types = vec!["PC".into(), "PS5".into()];
        let mut deduction = p.rules[0].clone();
        deduction.id = "speed".into();
        deduction.target = PricingTarget::Deduction;
        deduction.action = PricingAction::Multiplier { value: "2".into() };
        p.rules.push(deduction);
        assert!(PricingPolicyService::validate_policy(&p).is_ok());
        for device in ["PC", "PS5"] {
            assert_eq!(
                PricingPolicyService::evaluate_plan_price(
                    200.0,
                    Some(device),
                    &p,
                    at,
                    "Asia/Kolkata"
                )
                .unwrap(),
                250.0
            );
        }
        assert_eq!(
            PricingPolicyService::evaluate_plan_price(200.0, Some("OTHER"), &p, at, "Asia/Kolkata")
                .unwrap(),
            200.0
        );
        p.minimum_price = Some("50".into());
        let result = PricingPolicyService::evaluate(
            &p,
            &PricingSimulationDto {
                target: Some(PricingTarget::Deduction),
                location_id: None,
                device_type: Some("PS5".into()),
                at,
                base_rate: None,
                product_id: None,
                category: None,
            },
            "Asia/Kolkata",
            "INR",
        )
        .unwrap();
        assert_eq!(result.final_price, "2");
        assert_eq!(result.currency, "credits/min");
        assert_eq!(result.trace.len(), 1);
        p.rules[1].action = PricingAction::Fixed { value: "0".into() };
        assert!(PricingPolicyService::validate_policy(&p).is_err());
    }

    #[test]
    fn applies_midnight_spanning_rule_with_decimal_money() {
        let at = Utc.with_ymd_and_hms(2026, 9, 24, 18, 30, 0).unwrap(); // midnight IST
        let result = PricingPolicyService::evaluate(
            &policy(),
            &PricingSimulationDto {
                target: None,
                location_id: None,
                device_type: Some("PS5".to_string()),
                at,
                base_rate: None,
                product_id: None,
                category: None,
            },
            "Asia/Kolkata",
            "INR",
        )
        .unwrap();
        assert_eq!(result.final_price, "125");
        assert_eq!(result.trace.len(), 1);
    }

    #[test]
    fn leaves_non_matching_device_at_base_rate() {
        let at = Utc.with_ymd_and_hms(2026, 9, 24, 18, 30, 0).unwrap();
        let result = PricingPolicyService::evaluate(
            &policy(),
            &PricingSimulationDto {
                target: None,
                location_id: None,
                device_type: Some("PC".to_string()),
                at,
                base_rate: None,
                product_id: None,
                category: None,
            },
            "Asia/Kolkata",
            "INR",
        )
        .unwrap();
        assert_eq!(result.final_price, "100");
        assert!(result.trace.is_empty());
    }

    #[test]
    fn legacy_product_price_uses_the_typed_midnight_evaluator() {
        let at = Utc.with_ymd_and_hms(2026, 9, 24, 18, 30, 0).unwrap();
        let price = PricingPolicyService::evaluate_legacy_product_price(
            80.0,
            100.0,
            at,
            "Asia/Kolkata",
            "23:00",
            "08:00",
        )
        .unwrap();
        assert_eq!(price, 100.0);
    }

    fn product_rule(id: &str, priority: i32, action: PricingAction) -> PricingRule {
        PricingRule {
            id: id.to_string(),
            name: id.to_string(),
            priority,
            target: PricingTarget::Products,
            device_types: vec![],
            product_ids: vec![],
            categories: vec![],
            weekdays: vec![],
            start_time: None,
            end_time: None,
            starts_at: None,
            ends_at: None,
            action,
        }
    }

    #[test]
    fn product_rules_apply_to_all_products_or_only_their_scope() {
        let at = Utc.with_ymd_and_hms(2026, 9, 24, 6, 30, 0).unwrap(); // noon IST
        let burger = Uuid::new_v4();
        let mut meals = product_rule(
            "meals",
            10,
            PricingAction::Multiplier {
                value: "0.9".to_string(),
            },
        );
        meals.categories = vec!["meal".to_string()];
        let everything = product_rule(
            "everything",
            5,
            PricingAction::Multiplier {
                value: "1.05".to_string(),
            },
        );
        let rules = [meals, everything];
        let price = |id, category| {
            PricingPolicyService::evaluate_product_price(
                200.0,
                250.0,
                id,
                category,
                &rules,
                at,
                "Asia/Kolkata",
                "23:00",
                "08:00",
            )
            .unwrap()
        };
        assert_eq!(price(burger, "meal"), 189.0);
        assert_eq!(price(Uuid::new_v4(), "beverage"), 210.0);
    }

    #[test]
    fn session_and_product_rules_do_not_cross_over() {
        let at = Utc.with_ymd_and_hms(2026, 9, 24, 18, 30, 0).unwrap();
        let mut policy = policy();
        policy.rules[0].device_types.clear();
        let price = PricingPolicyService::evaluate_product_price(
            80.0,
            80.0,
            Uuid::new_v4(),
            "snack",
            &policy.rules,
            at,
            "Asia/Kolkata",
            "23:00",
            "08:00",
        )
        .unwrap();
        assert_eq!(price, 80.0);

        policy.rules = vec![product_rule(
            "free",
            1,
            PricingAction::Fixed {
                value: "0".to_string(),
            },
        )];
        let session = PricingPolicyService::evaluate(
            &policy,
            &PricingSimulationDto {
                target: None,
                location_id: None,
                device_type: None,
                at,
                base_rate: None,
                product_id: None,
                category: None,
            },
            "Asia/Kolkata",
            "INR",
        )
        .unwrap();
        assert_eq!(session.final_price, "100");
    }

    #[test]
    fn rejects_mixed_or_unknown_rule_scopes() {
        let mut rule = product_rule(
            "drinks",
            1,
            PricingAction::Multiplier {
                value: "1".to_string(),
            },
        );
        rule.categories = vec!["drinks".to_string()];
        let mut policy = policy();
        policy.rules = vec![rule.clone()];
        assert!(PricingPolicyService::validate_policy(&policy).is_err());

        rule.categories = vec!["beverage".to_string()];
        rule.device_types = vec!["PC".to_string()];
        policy.rules = vec![rule.clone()];
        assert!(PricingPolicyService::validate_policy(&policy).is_err());

        rule.device_types.clear();
        policy.rules = vec![rule];
        assert!(PricingPolicyService::validate_policy(&policy).is_ok());
    }
}

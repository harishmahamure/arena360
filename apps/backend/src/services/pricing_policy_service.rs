use std::str::FromStr;

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
    PricingTraceStep, PublishPricingRuleVersionDto,
};
use crate::repositories::PricingPolicyRepository;

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
        dto: CreatePricingRuleSetDto,
        actor_id: Uuid,
    ) -> Result<(PricingRuleSet, PricingRuleVersion), AppError> {
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
                dto.location_id,
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
            if rule_set
                .location_id
                .is_some_and(|scoped| scoped != requested)
            {
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
        let base = input
            .base_rate
            .as_deref()
            .map(|value| decimal("baseRate", value))
            .transpose()?
            .unwrap_or(decimal("baseRate", &policy.base_rate)?);
        let mut current = base;
        let mut trace = Vec::new();
        let mut rules: Vec<&PricingRule> = policy
            .rules
            .iter()
            .filter(|rule| {
                Self::matches(
                    rule,
                    input.device_type.as_deref(),
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
        if let Some(minimum) = policy.minimum_price.as_deref() {
            current = current.max(decimal("minimumPrice", minimum)?);
        }
        if let Some(maximum) = policy.maximum_price.as_deref() {
            current = current.min(decimal("maximumPrice", maximum)?);
        }
        current = current.round_dp(policy.rounding_scale);

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
            currency: currency.to_string(),
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
                device_types: vec![],
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
                location_id: None,
                device_type: None,
                at,
                base_rate: None,
            },
            timezone,
            "LEGACY",
        )?;
        result.final_price.parse::<f64>().map_err(|error| {
            AppError::Internal(format!("Legacy product price conversion failed: {error}"))
        })
    }

    fn matches(
        rule: &PricingRule,
        device_type: Option<&str>,
        at: chrono::DateTime<Utc>,
        local_time: chrono::NaiveTime,
        weekday: u8,
    ) -> bool {
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
                device_types: vec!["PS5".to_string()],
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
    fn applies_midnight_spanning_rule_with_decimal_money() {
        let at = Utc.with_ymd_and_hms(2026, 9, 24, 18, 30, 0).unwrap(); // midnight IST
        let result = PricingPolicyService::evaluate(
            &policy(),
            &PricingSimulationDto {
                location_id: None,
                device_type: Some("PS5".to_string()),
                at,
                base_rate: None,
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
                location_id: None,
                device_type: Some("PC".to_string()),
                at,
                base_rate: None,
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
}

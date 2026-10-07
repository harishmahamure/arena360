use chrono::Utc;
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, get_or_set, keys, CacheService};
use crate::error::AppError;
use crate::models::{
    parse_deduction_profile, parse_time, CreatePlanDto, Plan, PlanFilterDto, UpdatePlanDto,
};
use crate::repositories::{
    PlanCreateValues, PlanRepository, TenantPlanCreateValues, TenantPlanRepository,
    TenantPricingPolicyRepository,
};
use crate::services::{ConfigService, PricingPolicyService};
use crate::tenancy::TenantDb;
use crate::validation::{optional_device_sub_type, optional_device_type};

const VALID_DAYS: &[&str] = &[
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

pub struct PlanService {
    pool: PgPool,
    repo: PlanRepository,
    pricing: crate::services::PricingPolicyService,
    cache: Arc<dyn CacheService>,
    settings: Arc<ConfigService>,
}

impl PlanService {
    pub fn new(pool: PgPool, cache: Arc<dyn CacheService>, settings: Arc<ConfigService>) -> Self {
        Self {
            pool: pool.clone(),
            repo: PlanRepository::new(pool.clone()),
            pricing: crate::services::PricingPolicyService::new(pool),
            cache,
            settings,
        }
    }

    pub fn with_locations(mut self, ids: Vec<Uuid>) -> Self {
        self.repo = self.repo.with_locations(ids);
        self
    }

    async fn invalidate_plans(&self, id: Option<Uuid>) -> Result<(), AppError> {
        let mut cache_keys = vec![keys::plans_active().to_string()];
        if let Some(id) = id {
            cache_keys.push(keys::plan(&id));
        }
        cache::invalidate(&*self.cache, &cache_keys).await?;
        self.cache.invalidate_prefix("plans:list:").await
    }

    pub async fn list(
        &self,
        filters: PlanFilterDto,
    ) -> Result<crate::dto::PaginationResult<Plan>, AppError> {
        let cache_key = keys::plans_list(&keys::filter_hash(&filters));
        let mut result = get_or_set(&*self.cache, &cache_key, keys::ttl::LOOKUP, || async {
            self.repo.list(&filters).await
        })
        .await?;
        if filters.location_id.is_some() {
            self.price_plans(&mut result.data, filters.location_id)
                .await?;
        }
        Ok(result)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Plan, AppError> {
        self.get_by_id_for(id, None).await
    }

    pub async fn get_by_id_for(
        &self,
        id: Uuid,
        location_id: Option<Uuid>,
    ) -> Result<Plan, AppError> {
        let cache_key = keys::plan(&id);
        let mut plan = get_or_set(&*self.cache, &cache_key, keys::ttl::LOOKUP, || async {
            self.repo
                .find_by_id(id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Plan with ID {id} not found")))
        })
        .await?;
        if location_id.is_some() {
            self.price_plans(std::slice::from_mut(&mut plan), location_id)
                .await?;
        }
        Ok(plan)
    }

    pub async fn get_active(&self) -> Result<Vec<Plan>, AppError> {
        self.get_active_for(None).await
    }

    pub async fn get_active_for(&self, location_id: Option<Uuid>) -> Result<Vec<Plan>, AppError> {
        let mut plans = get_or_set(
            &*self.cache,
            keys::plans_active(),
            keys::ttl::LOOKUP,
            || async { self.repo.find_active().await },
        )
        .await?;
        let location = location_id.unwrap_or(crate::models::DEFAULT_VENUE_LOCATION_ID);
        let visible:Vec<Uuid>=sqlx::query_scalar(r#"SELECT p.id FROM plans p JOIN venue_locations l ON l.id=$1 AND l."organizationId"=p."organizationId"
            WHERE cardinality(p."locationIds")=0 OR l.id=ANY(p."locationIds")"#).bind(location).fetch_all(&self.pool).await?;
        plans.retain(|plan| visible.contains(&plan.id));
        self.price_plans(&mut plans, Some(location)).await?;
        Ok(plans)
    }

    async fn price_plans(
        &self,
        plans: &mut [Plan],
        location_id: Option<Uuid>,
    ) -> Result<(), AppError> {
        let location_id = location_id.unwrap_or(crate::models::DEFAULT_VENUE_LOCATION_ID);
        let policy = self
            .pricing
            .active_plan_policy_for(crate::models::DEFAULT_ORGANIZATION_ID, Some(location_id))
            .await?;
        let (timezone, _, _) = self
            .settings
            .venue_pricing_context(crate::models::DEFAULT_ORGANIZATION_ID, Some(location_id))
            .await?;
        let now = chrono::Utc::now();
        for plan in plans {
            let price = super::catalog_scope::available(&self.pool, "plans", plan.id, location_id)
                .await?
                .unwrap_or(plan.price);
            plan.current_price = Some(crate::services::PricingPolicyService::evaluate_plan_price(
                price,
                plan.device_type.as_deref(),
                &policy,
                now,
                &timezone,
            )?);
        }
        Ok(())
    }

    pub async fn create(
        &self,
        dto: CreatePlanDto,
        actor_id: Option<Uuid>,
    ) -> Result<Plan, AppError> {
        let validity_days = match dto.validity_days {
            Some(value) => value,
            None => self
                .settings
                .resolve_value(
                    crate::models::DEFAULT_ORGANIZATION_ID,
                    None,
                    "plans.default_validity_days",
                )
                .await?
                .as_i64()
                .unwrap_or(30) as i32,
        };
        let time_credits = match dto.time_credits {
            Some(value) => value,
            None => self
                .settings
                .resolve_value(
                    crate::models::DEFAULT_ORGANIZATION_ID,
                    None,
                    "plans.default_time_credits",
                )
                .await?
                .as_i64()
                .unwrap_or(60) as i32,
        };
        self.validate_create(&dto, validity_days, time_credits)?;

        let time_window_start = dto
            .time_window_start
            .as_deref()
            .map(parse_time)
            .transpose()?;
        let time_window_end = dto.time_window_end.as_deref().map(parse_time).transpose()?;
        let (dynamic_deduction_enabled, deduction_profile) = Self::resolve_deduction_fields(
            &dto.dynamic_deduction_enabled,
            dto.deduction_profile.as_ref(),
        )?;

        let plan = self
            .repo
            .create(
                PlanCreateValues {
                    dto: &dto,
                    validity_days,
                    time_credits,
                    time_window_start,
                    time_window_end,
                    dynamic_deduction_enabled,
                    deduction_profile,
                },
                actor_id,
            )
            .await?;
        self.invalidate_plans(Some(plan.id)).await?;
        Ok(plan)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: UpdatePlanDto,
        actor_id: Option<Uuid>,
    ) -> Result<Plan, AppError> {
        let existing = self.get_by_id(id).await?;

        if let Some(price) = dto.price {
            if price <= 0.0 {
                return Err(AppError::BadRequest(
                    "price must be greater than 0".to_string(),
                ));
            }
        }

        let existing_start = existing
            .time_window_start
            .map(|t| t.format("%H:%M:%S").to_string());
        let existing_end = existing
            .time_window_end
            .map(|t| t.format("%H:%M:%S").to_string());

        let plan_type = dto.plan_type.as_deref().unwrap_or(&existing.plan_type);
        self.validate_plan_type(
            plan_type,
            dto.time_credits.or(Some(existing.time_credits)),
            dto.validity_days.or(Some(existing.validity_days)),
            dto.time_window_start
                .as_deref()
                .or(existing_start.as_deref()),
            dto.time_window_end.as_deref().or(existing_end.as_deref()),
        )?;

        Self::validate_allowed_days(dto.allowed_days.as_ref().or(existing.allowed_days.as_ref()))?;
        Self::validate_allowed_months(
            dto.allowed_months
                .as_ref()
                .or(existing.allowed_months.as_ref()),
        )?;
        Self::validate_device_scope(
            dto.device_type
                .as_deref()
                .or(existing.device_type.as_deref()),
            dto.device_sub_type
                .as_deref()
                .or(existing.device_sub_type.as_deref()),
        )?;

        let time_window_start = match dto.time_window_start.as_deref() {
            Some(value) => Some(parse_time(value)?),
            None => None,
        };
        let time_window_end = match dto.time_window_end.as_deref() {
            Some(value) => Some(parse_time(value)?),
            None => None,
        };

        let dynamic_enabled = dto
            .dynamic_deduction_enabled
            .unwrap_or(existing.dynamic_deduction_enabled);
        let profile_value = if dto.dynamic_deduction_enabled == Some(false) {
            None
        } else {
            dto.deduction_profile
                .as_ref()
                .or(existing.deduction_profile.as_ref())
        };
        let (dynamic_deduction_enabled, deduction_profile) =
            Self::resolve_deduction_fields(&Some(dynamic_enabled), profile_value)?;

        let plan = self
            .repo
            .update(
                id,
                &dto,
                time_window_start,
                time_window_end,
                dto.allowed_days.as_ref(),
                dto.allowed_months.as_ref(),
                Some(dynamic_deduction_enabled),
                deduction_profile.as_ref(),
                actor_id,
            )
            .await?;
        self.invalidate_plans(Some(id)).await?;
        Ok(plan)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        self.get_by_id(id).await?;
        self.repo.deactivate(id).await?;
        self.invalidate_plans(Some(id)).await
    }

    pub async fn list_tenant(
        &self,
        db: Arc<TenantDb>,
        filters: PlanFilterDto,
    ) -> Result<crate::dto::PaginationResult<Plan>, AppError> {
        let repo = TenantPlanRepository::new(db.clone());
        let mut result = repo.list(&filters).await?;
        if let Some(location_id) = filters.location_id {
            self.price_tenant_plans(db, &repo, &mut result.data, location_id)
                .await?;
        }
        Ok(result)
    }

    pub async fn get_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
        location_id: Option<Uuid>,
    ) -> Result<Plan, AppError> {
        let repo = TenantPlanRepository::new(db.clone());
        let mut plan = repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Plan with ID {id} not found")))?;
        if let Some(location_id) = location_id {
            let (all, rows) = repo.location_scope(id).await?;
            if !all
                && !rows
                    .iter()
                    .any(|(available_location, _)| *available_location == location_id)
            {
                return Err(AppError::BadRequest(
                    "This catalog item is not available at the selected location".into(),
                ));
            }
            self.price_tenant_plans(db, &repo, std::slice::from_mut(&mut plan), location_id)
                .await?;
        }
        Ok(plan)
    }

    pub async fn active_tenant(
        &self,
        db: Arc<TenantDb>,
        location_id: Uuid,
    ) -> Result<Vec<Plan>, AppError> {
        let repo = TenantPlanRepository::new(db.clone());
        let mut plans = repo
            .list(&PlanFilterDto {
                location_id: Some(location_id),
                is_active: Some(1),
                limit: Some(100),
                ..Default::default()
            })
            .await?
            .data;
        self.price_tenant_plans(db, &repo, &mut plans, location_id)
            .await?;
        Ok(plans)
    }

    async fn price_tenant_plans(
        &self,
        db: Arc<TenantDb>,
        repo: &TenantPlanRepository,
        plans: &mut [Plan],
        location_id: Uuid,
    ) -> Result<(), AppError> {
        let pricing = TenantPricingPolicyRepository::new(db.clone());
        pricing.activate_due().await?;
        let values = pricing
            .active_policies(crate::models::DEFAULT_ORGANIZATION_ID, Some(location_id))
            .await?;
        let mut policy = crate::models::PricingPolicy {
            base_rate: "0".into(),
            rules: vec![],
            rounding_scale: 2,
            minimum_price: None,
            maximum_price: None,
        };
        for value in values {
            let next: crate::models::PricingPolicy =
                serde_json::from_value(value).map_err(|error| {
                    AppError::Internal(format!("Invalid published policy: {error}"))
                })?;
            policy.rules.extend(
                next.rules
                    .into_iter()
                    .filter(|rule| rule.target == crate::models::PricingTarget::Sessions),
            );
        }
        let (timezone, _, _) = self
            .settings
            .venue_pricing_context_tenant(
                db,
                crate::models::DEFAULT_ORGANIZATION_ID,
                Some(location_id),
            )
            .await?;
        for plan in plans {
            let base = repo
                .location_price(plan.id, location_id)
                .await?
                .unwrap_or(plan.price);
            plan.current_price = Some(PricingPolicyService::evaluate_plan_price(
                base,
                plan.device_type.as_deref(),
                &policy,
                Utc::now(),
                &timezone,
            )?);
        }
        Ok(())
    }

    pub async fn create_tenant(
        &self,
        db: Arc<TenantDb>,
        locations: Vec<Uuid>,
        dto: CreatePlanDto,
        actor_id: Option<Uuid>,
    ) -> Result<Plan, AppError> {
        let organization_id = db.tenant_id();
        let validity_days = match dto.validity_days {
            Some(value) => value,
            None => self
                .settings
                .resolve_value(organization_id, None, "plans.default_validity_days")
                .await?
                .as_i64()
                .unwrap_or(30) as i32,
        };
        let time_credits = match dto.time_credits {
            Some(value) => value,
            None => self
                .settings
                .resolve_value(organization_id, None, "plans.default_time_credits")
                .await?
                .as_i64()
                .unwrap_or(60) as i32,
        };
        self.validate_create(&dto, validity_days, time_credits)?;
        let start = dto
            .time_window_start
            .as_deref()
            .map(parse_time)
            .transpose()?;
        let end = dto.time_window_end.as_deref().map(parse_time).transpose()?;
        let (dynamic, profile) = Self::resolve_deduction_fields(
            &dto.dynamic_deduction_enabled,
            dto.deduction_profile.as_ref(),
        )?;
        let plan = TenantPlanRepository::new(db)
            .with_locations(locations)
            .create(
                TenantPlanCreateValues {
                    dto: &dto,
                    validity_days,
                    time_credits,
                    time_window_start: start,
                    time_window_end: end,
                    dynamic_deduction_enabled: dynamic,
                    deduction_profile: profile,
                },
                actor_id,
            )
            .await?;
        self.invalidate_plans(Some(plan.id)).await?;
        Ok(plan)
    }

    pub async fn delete_tenant(&self, db: Arc<TenantDb>, id: Uuid) -> Result<(), AppError> {
        TenantPlanRepository::new(db).deactivate(id).await?;
        self.invalidate_plans(Some(id)).await
    }

    pub async fn update_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
        dto: UpdatePlanDto,
        actor_id: Option<Uuid>,
    ) -> Result<Plan, AppError> {
        let existing = self.get_tenant(db.clone(), id, None).await?;
        if dto.price.is_some_and(|price| price <= 0.0) {
            return Err(AppError::BadRequest("price must be greater than 0".into()));
        }
        let existing_start = existing
            .time_window_start
            .map(|value| value.format("%H:%M:%S").to_string());
        let existing_end = existing
            .time_window_end
            .map(|value| value.format("%H:%M:%S").to_string());
        let plan_type = dto.plan_type.as_deref().unwrap_or(&existing.plan_type);
        self.validate_plan_type(
            plan_type,
            dto.time_credits.or(Some(existing.time_credits)),
            dto.validity_days.or(Some(existing.validity_days)),
            dto.time_window_start
                .as_deref()
                .or(existing_start.as_deref()),
            dto.time_window_end.as_deref().or(existing_end.as_deref()),
        )?;
        Self::validate_allowed_days(dto.allowed_days.as_ref().or(existing.allowed_days.as_ref()))?;
        Self::validate_allowed_months(
            dto.allowed_months
                .as_ref()
                .or(existing.allowed_months.as_ref()),
        )?;
        Self::validate_device_scope(
            dto.device_type
                .as_deref()
                .or(existing.device_type.as_deref()),
            dto.device_sub_type
                .as_deref()
                .or(existing.device_sub_type.as_deref()),
        )?;
        let start = dto
            .time_window_start
            .as_deref()
            .map(parse_time)
            .transpose()?;
        let end = dto.time_window_end.as_deref().map(parse_time).transpose()?;
        let dynamic = dto
            .dynamic_deduction_enabled
            .unwrap_or(existing.dynamic_deduction_enabled);
        let profile = if dto.dynamic_deduction_enabled == Some(false) {
            None
        } else {
            dto.deduction_profile
                .as_ref()
                .or(existing.deduction_profile.as_ref())
        };
        let (dynamic, profile) = Self::resolve_deduction_fields(&Some(dynamic), profile)?;
        let plan = TenantPlanRepository::new(db)
            .update(
                id,
                &dto,
                start,
                end,
                dto.allowed_days.as_ref(),
                dto.allowed_months.as_ref(),
                Some(dynamic),
                profile.as_ref(),
                actor_id,
            )
            .await?;
        self.invalidate_plans(Some(id)).await?;
        Ok(plan)
    }

    fn validate_create(
        &self,
        dto: &CreatePlanDto,
        validity_days: i32,
        time_credits: i32,
    ) -> Result<(), AppError> {
        if dto.price <= 0.0 {
            return Err(AppError::BadRequest(
                "price must be greater than 0".to_string(),
            ));
        }

        self.validate_plan_type(
            &dto.plan_type,
            Some(time_credits),
            Some(validity_days),
            dto.time_window_start.as_deref(),
            dto.time_window_end.as_deref(),
        )?;

        Self::validate_allowed_days(dto.allowed_days.as_ref())?;
        Self::validate_allowed_months(dto.allowed_months.as_ref())?;
        Self::validate_device_scope(dto.device_type.as_deref(), dto.device_sub_type.as_deref())?;
        Self::resolve_deduction_fields(
            &dto.dynamic_deduction_enabled,
            dto.deduction_profile.as_ref(),
        )?;

        Ok(())
    }

    fn resolve_deduction_fields(
        enabled: &Option<bool>,
        profile: Option<&Value>,
    ) -> Result<(bool, Option<Value>), AppError> {
        let enabled = enabled.unwrap_or(false);
        if !enabled {
            if profile.is_some() {
                return Err(AppError::BadRequest(
                    "deductionProfile must be omitted when dynamicDeductionEnabled is false"
                        .to_string(),
                ));
            }
            return Ok((false, None));
        }
        let Some(value) = profile else {
            return Err(AppError::BadRequest(
                "deductionProfile is required when dynamicDeductionEnabled is true".to_string(),
            ));
        };
        parse_deduction_profile(value)?;
        Ok((true, Some(value.clone())))
    }

    fn validate_device_scope(
        device_type: Option<&str>,
        device_sub_type: Option<&str>,
    ) -> Result<(), AppError> {
        let _ = optional_device_type(device_type.map(str::to_string))?;
        let _ = optional_device_sub_type(device_sub_type.map(str::to_string))?;
        Ok(())
    }

    fn validate_plan_type(
        &self,
        plan_type: &str,
        time_credits: Option<i32>,
        validity_days: Option<i32>,
        time_window_start: Option<&str>,
        time_window_end: Option<&str>,
    ) -> Result<(), AppError> {
        match plan_type {
            "time_based" | "weekend_special" => {}
            other => {
                return Err(AppError::BadRequest(format!(
                    "Only time_based and weekend_special (Happy Hours) plan types are supported, got '{other}'"
                )));
            }
        }

        if time_credits.unwrap_or(0) <= 0 {
            return Err(AppError::BadRequest(
                "Plans require timeCredits > 0".to_string(),
            ));
        }

        if validity_days.unwrap_or(0) <= 0 {
            return Err(AppError::BadRequest(
                "Plans require validityDays > 0".to_string(),
            ));
        }

        if plan_type == "weekend_special"
            && (time_window_start.is_none() || time_window_end.is_none())
        {
            return Err(AppError::BadRequest(
                "Happy Hours plans require both timeWindowStart and timeWindowEnd".to_string(),
            ));
        }

        if let (Some(start), Some(end)) = (time_window_start, time_window_end) {
            if start >= end {
                return Err(AppError::BadRequest(
                    "timeWindowStart must be less than timeWindowEnd".to_string(),
                ));
            }
        }

        Ok(())
    }

    fn validate_allowed_days(value: Option<&Value>) -> Result<(), AppError> {
        let Some(val) = value else { return Ok(()) };
        let arr = val.as_array().ok_or_else(|| {
            AppError::BadRequest("allowedDays must be a JSON array of day names".to_string())
        })?;
        for item in arr {
            let day = item.as_str().ok_or_else(|| {
                AppError::BadRequest("Each allowedDays entry must be a string".to_string())
            })?;
            if !VALID_DAYS.contains(&day) {
                return Err(AppError::BadRequest(format!(
                    "Invalid day name '{day}'. Valid: monday..sunday"
                )));
            }
        }
        Ok(())
    }

    fn validate_allowed_months(value: Option<&Value>) -> Result<(), AppError> {
        let Some(val) = value else { return Ok(()) };
        let arr = val.as_array().ok_or_else(|| {
            AppError::BadRequest("allowedMonths must be a JSON array of month numbers".to_string())
        })?;
        for item in arr {
            let month = item.as_i64().ok_or_else(|| {
                AppError::BadRequest("Each allowedMonths entry must be an integer".to_string())
            })?;
            if !(1..=12).contains(&month) {
                return Err(AppError::BadRequest(format!(
                    "Invalid month {month}. Must be 1-12"
                )));
            }
        }
        Ok(())
    }
}

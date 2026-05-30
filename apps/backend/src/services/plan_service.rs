use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{parse_time, CreatePlanDto, Plan, PlanFilterDto, UpdatePlanDto};
use crate::repositories::{PlanCreateValues, PlanRepository};
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
    repo: PlanRepository,
}

impl PlanService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: PlanRepository::new(pool),
        }
    }

    pub async fn list(
        &self,
        filters: PlanFilterDto,
    ) -> Result<crate::dto::PaginationResult<Plan>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Plan, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Plan with ID {id} not found")))
    }

    pub async fn get_active(&self) -> Result<Vec<Plan>, AppError> {
        self.repo.find_active().await
    }

    pub async fn create(
        &self,
        dto: CreatePlanDto,
        actor_id: Option<Uuid>,
    ) -> Result<Plan, AppError> {
        self.validate_create(&dto)?;

        let validity_days = dto.validity_days.unwrap_or(30);
        let time_credits = dto.time_credits.unwrap_or(60);

        let time_window_start = dto
            .time_window_start
            .as_deref()
            .map(parse_time)
            .transpose()?;
        let time_window_end = dto.time_window_end.as_deref().map(parse_time).transpose()?;

        self.repo
            .create(
                PlanCreateValues {
                    dto: &dto,
                    validity_days,
                    time_credits,
                    time_window_start,
                    time_window_end,
                },
                actor_id,
            )
            .await
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

        self.repo
            .update(
                id,
                &dto,
                time_window_start,
                time_window_end,
                dto.allowed_days.as_ref(),
                dto.allowed_months.as_ref(),
                actor_id,
            )
            .await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        self.get_by_id(id).await?;
        self.repo.deactivate(id).await
    }

    fn validate_create(&self, dto: &CreatePlanDto) -> Result<(), AppError> {
        if dto.price <= 0.0 {
            return Err(AppError::BadRequest(
                "price must be greater than 0".to_string(),
            ));
        }

        let validity_days = dto.validity_days.unwrap_or(30);
        let time_credits = dto.time_credits.unwrap_or(60);

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

        Ok(())
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

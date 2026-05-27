use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    parse_time, CreatePlanDto, Plan, PlanFilterDto, UpdatePlanDto,
};
use crate::repositories::{PlanCreateValues, PlanRepository};

struct PlanTypeValidation<'a> {
    plan_type: &'a str,
    duration_minutes: Option<i32>,
    time_credits: Option<i32>,
    max_sessions: Option<i32>,
    validity_days: Option<i32>,
    per_minute_rate: Option<f64>,
    time_window_start: Option<&'a str>,
    time_window_end: Option<&'a str>,
}

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

    pub async fn create(&self, dto: CreatePlanDto) -> Result<Plan, AppError> {
        self.validate_create(&dto)?;

        let duration_minutes = dto.duration_minutes.unwrap_or(60);
        let validity_days = dto.validity_days.unwrap_or(30);
        let time_credits = dto.time_credits.unwrap_or(duration_minutes);
        let per_minute_rate = dto.per_minute_rate.unwrap_or(1.0);

        let time_window_start = dto
            .time_window_start
            .as_deref()
            .map(parse_time)
            .transpose()?;
        let time_window_end = dto
            .time_window_end
            .as_deref()
            .map(parse_time)
            .transpose()?;

        self.repo
            .create(PlanCreateValues {
                dto: &dto,
                duration_minutes,
                validity_days,
                time_credits,
                per_minute_rate,
                time_window_start,
                time_window_end,
            })
            .await
    }

    pub async fn update(&self, id: Uuid, dto: UpdatePlanDto) -> Result<Plan, AppError> {
        let existing = self.get_by_id(id).await?;

        if let Some(price) = dto.price {
            if price <= 0.0 {
                return Err(AppError::BadRequest("price must be greater than 0".to_string()));
            }
        }

        if let Some(rate) = dto.per_minute_rate {
            if rate <= 0.0 {
                return Err(AppError::BadRequest(
                    "perMinuteRate must be greater than 0".to_string(),
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
        self.validate_plan_type(PlanTypeValidation {
            plan_type,
            duration_minutes: dto.duration_minutes.or(Some(existing.duration_minutes)),
            time_credits: dto.time_credits.or(Some(existing.time_credits)),
            max_sessions: dto.max_sessions.or(existing.max_sessions),
            validity_days: dto.validity_days.or(Some(existing.validity_days)),
            per_minute_rate: dto.per_minute_rate.or(Some(existing.per_minute_rate)),
            time_window_start: dto.time_window_start.as_deref().or(existing_start.as_deref()),
            time_window_end: dto.time_window_end.as_deref().or(existing_end.as_deref()),
        })?;

        let time_window_start = match dto.time_window_start.as_deref() {
            Some(value) => Some(parse_time(value)?),
            None => None,
        };
        let time_window_end = match dto.time_window_end.as_deref() {
            Some(value) => Some(parse_time(value)?),
            None => None,
        };

        self.repo
            .update(id, &dto, time_window_start, time_window_end)
            .await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        self.get_by_id(id).await?;
        self.repo.deactivate(id).await
    }

    fn validate_create(&self, dto: &CreatePlanDto) -> Result<(), AppError> {
        if dto.price <= 0.0 {
            return Err(AppError::BadRequest("price must be greater than 0".to_string()));
        }

        let per_minute_rate = dto.per_minute_rate.unwrap_or(1.0);
        if per_minute_rate <= 0.0 {
            return Err(AppError::BadRequest(
                "perMinuteRate must be greater than 0".to_string(),
            ));
        }

        let duration_minutes = dto.duration_minutes.unwrap_or(60);
        let validity_days = dto.validity_days.unwrap_or(30);
        let time_credits = dto.time_credits.unwrap_or(duration_minutes);

        self.validate_plan_type(PlanTypeValidation {
            plan_type: &dto.plan_type,
            duration_minutes: Some(duration_minutes),
            time_credits: Some(time_credits),
            max_sessions: dto.max_sessions,
            validity_days: Some(validity_days),
            per_minute_rate: Some(per_minute_rate),
            time_window_start: dto.time_window_start.as_deref(),
            time_window_end: dto.time_window_end.as_deref(),
        })
    }

    fn validate_plan_type(&self, input: PlanTypeValidation<'_>) -> Result<(), AppError> {
        let PlanTypeValidation {
            plan_type,
            duration_minutes,
            time_credits,
            max_sessions,
            validity_days,
            per_minute_rate,
            time_window_start,
            time_window_end,
        } = input;
        match plan_type {
            "time_based" if duration_minutes.unwrap_or(0) <= 0 => {
                return Err(AppError::BadRequest(
                    "time_based plans require durationMinutes > 0".to_string(),
                ));
            }
            "time_based" if time_credits.unwrap_or(0) <= 0 => {
                return Err(AppError::BadRequest(
                    "time_based plans require timeCredits > 0".to_string(),
                ));
            }
            "session_based" if max_sessions.unwrap_or(0) <= 0 => {
                return Err(AppError::BadRequest(
                    "session_based plans require maxSessions > 0".to_string(),
                ));
            }
            "unlimited_daily" if validity_days.unwrap_or(0) <= 0 => {
                return Err(AppError::BadRequest(
                    "unlimited_daily plans require validityDays > 0".to_string(),
                ));
            }
            "hourly_rental" if per_minute_rate.unwrap_or(0.0) <= 0.0 => {
                return Err(AppError::BadRequest(
                    "hourly_rental plans require perMinuteRate > 0".to_string(),
                ));
            }
            "weekend_special"
                if time_window_start.is_none() || time_window_end.is_none() =>
            {
                return Err(AppError::BadRequest(
                    "weekend_special plans require both timeWindowStart and timeWindowEnd"
                        .to_string(),
                ));
            }
            _ => {}
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
}

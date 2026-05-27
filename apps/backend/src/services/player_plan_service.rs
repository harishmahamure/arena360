use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    status, AssignPlanDto, Plan, PlayerPlan, PlayerPlanCreateValues, PlayerPlanFilterDto,
    PlayerPlanUpdateValues, ValidationResult,
};
use crate::repositories::{PlanRepository, PlayerPlanRepository, UserRepository};

pub struct PlayerPlanService {
    repo: PlayerPlanRepository,
    plan_repo: PlanRepository,
    user_repo: UserRepository,
}

impl PlayerPlanService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: PlayerPlanRepository::new(pool.clone()),
            plan_repo: PlanRepository::new(pool.clone()),
            user_repo: UserRepository::new(pool),
        }
    }

    pub async fn list(
        &self,
        filters: PlayerPlanFilterDto,
    ) -> Result<crate::dto::PaginationResult<PlayerPlan>, AppError> {
        let mut result = self.repo.list(&filters).await?;
        self.expire_stale_in_results(&mut result.data).await?;
        Ok(result)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<PlayerPlan, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Player plan with ID {id} not found")))
    }

    pub async fn assign_plan_to_player(&self, dto: AssignPlanDto) -> Result<PlayerPlan, AppError> {
        let plan = self
            .plan_repo
            .find_by_id(dto.plan_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Plan with ID {} not found", dto.plan_id)))?;

        if !plan.is_active {
            return Err(AppError::BadRequest(
                "Cannot assign an inactive plan".to_string(),
            ));
        }

        let user = self
            .user_repo
            .find_by_id(dto.player_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("User with ID {} not found", dto.player_id))
            })?;

        if !user.is_active {
            return Err(AppError::BadRequest("User is not active".to_string()));
        }

        let purchase_date = dto.purchase_date.unwrap_or_else(Utc::now);
        let expiry_date = purchase_date + Duration::days(plan.validity_days as i64);

        let remaining_usage_count = if plan.plan_type == "session_based" {
            plan.max_sessions
        } else {
            None
        };

        let remaining_time_credits = Some(plan.time_credits);

        self.repo
            .create(&PlayerPlanCreateValues {
                player_id: dto.player_id,
                plan_id: dto.plan_id,
                purchase_date,
                expiry_date,
                remaining_usage_count,
                remaining_time_credits,
                status: status::ACTIVE.to_string(),
            })
            .await
    }

    pub async fn validate_plan_access(
        &self,
        player_plan_id: Uuid,
        current_time: Option<DateTime<Utc>>,
    ) -> Result<ValidationResult, AppError> {
        let player_plan = self.get_by_id(player_plan_id).await?;
        let plan = self
            .plan_repo
            .find_by_id(player_plan.plan_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "Associated plan with ID {} not found",
                    player_plan.plan_id
                ))
            })?;

        Ok(Self::validate_player_plan(&player_plan, &plan, current_time))
    }

    pub async fn deduct_time_credits(
        &self,
        player_plan_id: Uuid,
        credits: i32,
    ) -> Result<PlayerPlan, AppError> {
        let player_plan = self.get_by_id(player_plan_id).await?;

        let remaining = player_plan.remaining_time_credits.ok_or_else(|| {
            AppError::BadRequest("This plan does not have time credits".to_string())
        })?;

        let new_credits = remaining - credits;
        let mut update = PlayerPlanUpdateValues {
            remaining_time_credits: Some(new_credits),
            status: None,
            remaining_usage_count: None,
            activation_date: None,
        };

        if new_credits <= 0 {
            update.status = Some(status::EXHAUSTED.to_string());
        }

        self.repo.update(player_plan_id, &update).await
    }

    pub async fn deduct_session_count(&self, player_plan_id: Uuid) -> Result<PlayerPlan, AppError> {
        let player_plan = self.get_by_id(player_plan_id).await?;

        let remaining = player_plan.remaining_usage_count.ok_or_else(|| {
            AppError::BadRequest("This plan does not have session limits".to_string())
        })?;

        if remaining <= 0 {
            return Err(AppError::BadRequest("No sessions remaining".to_string()));
        }

        let new_count = remaining - 1;
        let mut update = PlayerPlanUpdateValues {
            remaining_usage_count: Some(new_count),
            status: None,
            remaining_time_credits: None,
            activation_date: None,
        };

        if new_count <= 0 {
            update.status = Some(status::EXHAUSTED.to_string());
        }

        self.repo.update(player_plan_id, &update).await
    }

    pub async fn get_best_plan(&self, player_id: Uuid) -> Result<PlayerPlan, AppError> {
        let mut result = self
            .list(PlayerPlanFilterDto {
                player_id: Some(player_id),
                status: Some(status::ACTIVE.to_string()),
                ..Default::default()
            })
            .await?;

        if result.data.is_empty() {
            return Err(AppError::NotFound(format!(
                "No active player plans found for player {player_id}"
            )));
        }

        result.data.sort_by(|a, b| {
            b.remaining_time_credits
                .unwrap_or(0)
                .cmp(&a.remaining_time_credits.unwrap_or(0))
        });

        Ok(result.data.remove(0))
    }

    pub fn enforce_player_scope(
        filters: PlayerPlanFilterDto,
        user_id: &str,
        is_admin: bool,
    ) -> Result<PlayerPlanFilterDto, AppError> {
        if is_admin {
            return Ok(filters);
        }

        let user_uuid = Uuid::parse_str(user_id)
            .map_err(|_| AppError::Unauthorized("Authentication required".to_string()))?;

        if let Some(player_id) = filters.player_id {
            if player_id != user_uuid {
                return Err(AppError::Forbidden(
                    "You can only access your own player plans".to_string(),
                ));
            }
        }

        Ok(PlayerPlanFilterDto {
            player_id: Some(user_uuid),
            ..filters
        })
    }

    pub fn ensure_owner_or_admin(
        claims_user_id: &str,
        is_admin: bool,
        player_plan: &PlayerPlan,
    ) -> Result<(), AppError> {
        if is_admin {
            return Ok(());
        }

        let user_uuid = Uuid::parse_str(claims_user_id)
            .map_err(|_| AppError::Unauthorized("Authentication required".to_string()))?;

        if player_plan.player_id != user_uuid {
            return Err(AppError::Forbidden(
                "You can only access your own player plans".to_string(),
            ));
        }

        Ok(())
    }

    async fn expire_stale_in_results(&self, player_plans: &mut [PlayerPlan]) -> Result<(), AppError> {
        let now = Utc::now();
        for player_plan in player_plans.iter_mut() {
            if player_plan.status == status::ACTIVE && player_plan.expiry_date < now {
                let _ = self
                    .repo
                    .update(
                        player_plan.id,
                        &PlayerPlanUpdateValues {
                            status: Some(status::EXPIRED.to_string()),
                            remaining_time_credits: None,
                            remaining_usage_count: None,
                            activation_date: None,
                        },
                    )
                    .await;
                player_plan.status = status::EXPIRED.to_string();
            }
        }
        Ok(())
    }

    fn validate_player_plan(
        player_plan: &PlayerPlan,
        plan: &Plan,
        current_time: Option<DateTime<Utc>>,
    ) -> ValidationResult {
        let now = current_time.unwrap_or_else(Utc::now);

        if player_plan.status != status::ACTIVE {
            return ValidationResult {
                valid: false,
                reason: Some(format!("Plan is {}", player_plan.status)),
            };
        }

        if now > player_plan.expiry_date {
            return ValidationResult {
                valid: false,
                reason: Some("Plan expired".to_string()),
            };
        }

        if let Some(credits) = player_plan.remaining_time_credits {
            if credits <= 0 {
                return ValidationResult {
                    valid: false,
                    reason: Some("Insufficient credits".to_string()),
                };
            }
        }

        if let Some(count) = player_plan.remaining_usage_count {
            if count <= 0 {
                return ValidationResult {
                    valid: false,
                    reason: Some("No sessions remaining".to_string()),
                };
            }
        }

        if let (Some(start), Some(end)) = (plan.time_window_start, plan.time_window_end) {
            let current = now.time();
            if current < start || current > end {
                return ValidationResult {
                    valid: false,
                    reason: Some(format!(
                        "Outside allowed time window ({} - {})",
                        start.format("%H:%M:%S"),
                        end.format("%H:%M:%S")
                    )),
                };
            }
        }

        ValidationResult {
            valid: true,
            reason: None,
        }
    }
}

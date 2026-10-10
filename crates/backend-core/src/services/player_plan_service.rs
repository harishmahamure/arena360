use crate::error::AppError;
use crate::models::{
    status, AssignPlanDto, Plan, PlayerPlan, PlayerPlanCreateValues, PlayerPlanFilterDto,
    PlayerPlanResponse, PlayerPlanUpdateValues, ValidationResult,
};
use crate::repositories::{TenantPlanRepository, TenantPlayerPlanRepository, TenantUserRepository};
use crate::tenancy::TenantDb;
use chrono::{DateTime, Duration, Utc};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Default)]
pub struct PlayerPlanService;
impl PlayerPlanService {
    pub fn new() -> Self {
        Self
    }
    pub async fn list_tenant(
        &self,
        db: Arc<TenantDb>,
        filters: PlayerPlanFilterDto,
    ) -> Result<crate::dto::PaginationResult<PlayerPlanResponse>, AppError> {
        let repo = TenantPlayerPlanRepository::new(db);
        let mut result = repo.list(&filters).await?;
        let now = Utc::now();
        for player_plan in &mut result.data {
            if player_plan.status == status::ACTIVE && player_plan.expiry_date < now {
                repo.update(
                    player_plan.id,
                    &PlayerPlanUpdateValues {
                        status: Some(status::EXPIRED.into()),
                        remaining_time_credits: None,
                        remaining_usage_count: None,
                        activation_date: None,
                    },
                    None,
                )
                .await?;
                player_plan.status = status::EXPIRED.into();
            }
        }
        Ok(result)
    }

    pub async fn get_by_id_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
    ) -> Result<PlayerPlanResponse, AppError> {
        TenantPlayerPlanRepository::new(db)
            .find_by_id(id)
            .await?
            .map(Into::into)
            .ok_or_else(|| AppError::NotFound(format!("Player plan with ID {id} not found")))
    }

    pub async fn assign_plan_to_player_tenant(
        &self,
        db: Arc<TenantDb>,
        dto: AssignPlanDto,
        actor_id: Option<Uuid>,
    ) -> Result<PlayerPlan, AppError> {
        let plan = TenantPlanRepository::new(db.clone())
            .find_by_id(dto.plan_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Plan with ID {} not found", dto.plan_id)))?;
        if !plan.is_active {
            return Err(AppError::BadRequest(
                "Cannot assign an inactive plan".into(),
            ));
        }
        TenantUserRepository::new(db.clone())
            .require_active_player(dto.player_id)
            .await?;
        let purchase_date = dto.purchase_date.unwrap_or_else(Utc::now);
        TenantPlayerPlanRepository::new(db)
            .create(
                &PlayerPlanCreateValues {
                    player_id: dto.player_id,
                    plan_id: dto.plan_id,
                    purchase_date,
                    expiry_date: purchase_date + Duration::days(i64::from(plan.validity_days)),
                    remaining_usage_count: None,
                    remaining_time_credits: Some(plan.time_credits),
                    status: status::ACTIVE.into(),
                },
                actor_id,
            )
            .await
    }

    pub async fn validate_plan_access_tenant(
        &self,
        db: Arc<TenantDb>,
        player_plan_id: Uuid,
        current_time: Option<DateTime<Utc>>,
    ) -> Result<ValidationResult, AppError> {
        let player_plan = self.get_by_id_tenant(db.clone(), player_plan_id).await?;
        let plan = TenantPlanRepository::new(db)
            .find_by_id(player_plan.plan_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "Associated plan with ID {} not found",
                    player_plan.plan_id
                ))
            })?;
        Ok(Self::validate_player_plan_response(
            &player_plan,
            &plan,
            current_time,
        ))
    }

    pub async fn deduct_time_credits_tenant(
        &self,
        db: Arc<TenantDb>,
        player_plan_id: Uuid,
        credits: i32,
    ) -> Result<PlayerPlan, AppError> {
        if credits <= 0 {
            return Err(AppError::BadRequest("Credits must be positive".into()));
        }
        let plan = TenantPlayerPlanRepository::new(db.clone())
            .find_by_id(player_plan_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Player plan with ID {player_plan_id} not found"))
            })?;
        let remaining = plan
            .remaining_time_credits
            .ok_or_else(|| AppError::BadRequest("This plan does not have time credits".into()))?;
        let next = (remaining - credits).max(0);
        TenantPlayerPlanRepository::new(db)
            .update(
                player_plan_id,
                &PlayerPlanUpdateValues {
                    remaining_time_credits: Some(next),
                    status: (next == 0).then(|| status::EXHAUSTED.into()),
                    remaining_usage_count: None,
                    activation_date: None,
                },
                None,
            )
            .await
    }

    pub async fn deduct_session_count_tenant(
        &self,
        db: Arc<TenantDb>,
        player_plan_id: Uuid,
    ) -> Result<PlayerPlan, AppError> {
        let plan = TenantPlayerPlanRepository::new(db.clone())
            .find_by_id(player_plan_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Player plan with ID {player_plan_id} not found"))
            })?;
        let remaining = plan
            .remaining_usage_count
            .ok_or_else(|| AppError::BadRequest("This plan does not have session limits".into()))?;
        if remaining <= 0 {
            return Err(AppError::BadRequest("No sessions remaining".into()));
        }
        let next = remaining - 1;
        TenantPlayerPlanRepository::new(db)
            .update(
                player_plan_id,
                &PlayerPlanUpdateValues {
                    remaining_usage_count: Some(next),
                    status: (next == 0).then(|| status::EXHAUSTED.into()),
                    remaining_time_credits: None,
                    activation_date: None,
                },
                None,
            )
            .await
    }

    pub async fn get_best_plan_tenant(
        &self,
        db: Arc<TenantDb>,
        player_id: Uuid,
    ) -> Result<PlayerPlan, AppError> {
        let result = self
            .list_tenant(
                db.clone(),
                PlayerPlanFilterDto {
                    player_id: Some(player_id),
                    status: Some(status::ACTIVE.into()),
                    limit: Some(100),
                    ..Default::default()
                },
            )
            .await?;
        let id = result
            .data
            .into_iter()
            .max_by_key(|plan| plan.remaining_time_credits.unwrap_or(0))
            .map(|plan| plan.id)
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "No active player plans found for player {player_id}"
                ))
            })?;
        TenantPlayerPlanRepository::new(db)
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Player plan with ID {id} not found")))
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
        player_id: Uuid,
    ) -> Result<(), AppError> {
        if is_admin {
            return Ok(());
        }

        let user_uuid = Uuid::parse_str(claims_user_id)
            .map_err(|_| AppError::Unauthorized("Authentication required".to_string()))?;

        if player_id != user_uuid {
            return Err(AppError::Forbidden(
                "You can only access your own player plans".to_string(),
            ));
        }

        Ok(())
    }

    fn validate_player_plan_response(
        player_plan: &PlayerPlanResponse,
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

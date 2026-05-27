use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    CreateSessionDto, EndSessionDto, SessionFilterDto, UpdateDeviceStatusDto, UsageSession,
};
use crate::repositories::{PlanRepository, SessionRepository};
use crate::services::{DeviceService, EventService, PlayerPlanService};

pub struct SessionService {
    repo: SessionRepository,
    plan_repo: PlanRepository,
    devices: DeviceService,
    player_plans: Arc<PlayerPlanService>,
    events: EventService,
}

impl SessionService {
    pub fn new(
        pool: PgPool,
        devices: DeviceService,
        player_plans: Arc<PlayerPlanService>,
        events: EventService,
    ) -> Self {
        Self {
            repo: SessionRepository::new(pool.clone()),
            plan_repo: PlanRepository::new(pool),
            devices,
            player_plans,
            events,
        }
    }

    pub async fn list(
        &self,
        filters: SessionFilterDto,
    ) -> Result<crate::dto::PaginationResult<UsageSession>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn list_active(
        &self,
    ) -> Result<crate::dto::PaginationResult<UsageSession>, AppError> {
        self.repo
            .list(&SessionFilterDto {
                is_active: Some(1),
                ..Default::default()
            })
            .await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<UsageSession, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Session with ID {id} not found")))
    }

    pub async fn start(&self, dto: CreateSessionDto) -> Result<UsageSession, AppError> {
        let active = self.repo.find_active_by_player_plan(dto.player_plan_id).await?;
        if !active.is_empty() {
            return Err(AppError::Conflict(format!(
                "Player plan already has an active session (ID: {})",
                active[0].id
            )));
        }

        let start_time = dto.start_time.unwrap_or_else(Utc::now);
        let validation = self
            .player_plans
            .validate_plan_access(dto.player_plan_id, Some(start_time))
            .await?;

        if !validation.valid {
            return Err(AppError::Forbidden(format!(
                "Cannot start session: {}",
                validation.reason.unwrap_or_else(|| "Plan access denied".to_string())
            )));
        }

        let device = self.devices.get_by_id(dto.device_id).await?;
        if device.status != "available" && device.status != "operational" {
            return Err(AppError::BadRequest(format!(
                "Device '{}' is not available (status: {})",
                device.name, device.status
            )));
        }

        let session = self.repo.create(&dto, start_time).await?;

        let _ = self
            .devices
            .update_status(
                dto.device_id,
                UpdateDeviceStatusDto {
                    status: "in_use".to_string(),
                },
            )
            .await;

        self.events
            .publish_session_started(&session.id.to_string());

        Ok(session)
    }

    pub async fn end(&self, id: Uuid, dto: EndSessionDto) -> Result<UsageSession, AppError> {
        let session = self.get_by_id(id).await?;

        if session.end_time.is_some() {
            return Err(AppError::BadRequest(
                "Session has already been ended".to_string(),
            ));
        }

        let end_time = dto.end_time.unwrap_or_else(Utc::now);
        let duration_ms = end_time
            .signed_duration_since(session.start_time)
            .num_milliseconds();
        let duration_minutes = ((duration_ms as f64) / (1000.0 * 60.0)).ceil() as i32;
        let duration_minutes = duration_minutes.max(0);

        let player_plan = self.player_plans.get_by_id(session.player_plan_id).await?;
        let plan = self
            .plan_repo
            .find_by_id(player_plan.plan_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Plan with ID {} not found", player_plan.plan_id))
            })?;

        let time_credits_consumed = dto.time_credits_consumed.unwrap_or_else(|| {
            ((duration_minutes as f64) * plan.per_minute_rate).ceil() as i32
        });

        let updated = self
            .repo
            .end(id, end_time, duration_minutes, Some(time_credits_consumed))
            .await?;

        if player_plan.remaining_time_credits.is_some() {
            if let Err(error) = self
                .player_plans
                .deduct_time_credits(session.player_plan_id, time_credits_consumed)
                .await
            {
                tracing::warn!("Failed to deduct time credits: {error}");
            }
        }

        if plan.plan_type == "session_based" {
            if let Err(error) = self
                .player_plans
                .deduct_session_count(session.player_plan_id)
                .await
            {
                tracing::warn!("Failed to deduct session count: {error}");
            }
        }

        let _ = self
            .devices
            .update_status(
                session.device_id,
                UpdateDeviceStatusDto {
                    status: "available".to_string(),
                },
            )
            .await;

        self.events
            .publish_session_ended(&updated.id.to_string());

        Ok(updated)
    }
}

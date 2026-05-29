use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    CreateSessionDto, EndSessionDto, SessionFilterDto, UpdateDeviceStatusDto, UsageSession,
    UsageSessionResponse,
};
use crate::realtime::OutboxService;
use crate::repositories::SessionRepository;
use crate::services::{BalanceService, DeviceService, EventService};

pub struct SessionService {
    repo: SessionRepository,
    devices: DeviceService,
    balances: Arc<BalanceService>,
    events: EventService,
    outbox: OutboxService,
}

impl SessionService {
    pub fn new(
        pool: PgPool,
        devices: DeviceService,
        balances: Arc<BalanceService>,
        events: EventService,
        outbox: OutboxService,
    ) -> Self {
        Self {
            repo: SessionRepository::new(pool),
            devices,
            balances,
            events,
            outbox,
        }
    }

    pub async fn list(
        &self,
        filters: SessionFilterDto,
    ) -> Result<crate::dto::PaginationResult<UsageSessionResponse>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn list_active(
        &self,
    ) -> Result<crate::dto::PaginationResult<UsageSessionResponse>, AppError> {
        self.repo
            .list(&SessionFilterDto {
                is_active: Some(1),
                ..Default::default()
            })
            .await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<UsageSessionResponse, AppError> {
        self.repo
            .find_enriched_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Session with ID {id} not found")))
    }

    pub async fn start(
        &self,
        dto: CreateSessionDto,
        actor_id: Option<Uuid>,
    ) -> Result<UsageSession, AppError> {
        let active = self
            .repo
            .find_active_by_balance(dto.balance_id)
            .await?;
        if !active.is_empty() {
            return Err(AppError::Conflict(format!(
                "Balance already has an active session (ID: {})",
                active[0].id
            )));
        }

        let start_time = dto.start_time.unwrap_or_else(Utc::now);
        let validation = self
            .balances
            .validate_access(dto.balance_id, Some(start_time))
            .await?;

        if !validation.valid {
            return Err(AppError::Forbidden(format!(
                "Cannot start session: {}",
                validation
                    .reason
                    .unwrap_or_else(|| "Balance access denied".to_string())
            )));
        }

        let device = self.devices.get_by_id(dto.device_id).await?;
        if device.status != "available" && device.status != "operational" {
            return Err(AppError::BadRequest(format!(
                "Device '{}' is not available (status: {})",
                device.name, device.status
            )));
        }

        let session = self.repo.create(&dto, start_time, actor_id).await?;

        let _ = self
            .devices
            .update_status(
                dto.device_id,
                UpdateDeviceStatusDto {
                    status: "in_use".to_string(),
                },
            )
            .await;

        self.events.publish_session_started(&session.id.to_string());

        let device_channel = format!("device:{}", dto.device_id);
        let payload = serde_json::json!({ "session_id": session.id.to_string(), "device_id": dto.device_id.to_string() });
        let _ = self.outbox.publish("staff", "session.started", payload.clone(), None, None, true).await;
        let _ = self.outbox.publish(&device_channel, "session.started", payload, None, None, false).await;

        Ok(session)
    }

    pub async fn end(
        &self,
        id: Uuid,
        dto: EndSessionDto,
        actor_id: Option<Uuid>,
    ) -> Result<UsageSession, AppError> {
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

        let balance_id = session.balance_id.ok_or_else(|| {
            AppError::BadRequest("Session has no linked balance (legacy session)".to_string())
        })?;

        let time_credits_consumed = dto
            .time_credits_consumed
            .unwrap_or(duration_minutes);

        let updated = self
            .repo
            .end(
                id,
                end_time,
                duration_minutes,
                Some(time_credits_consumed),
                actor_id,
            )
            .await?;

        if let Err(error) = self
            .balances
            .deduct_minutes(balance_id, time_credits_consumed, Some(updated.id))
            .await
        {
            tracing::warn!("Failed to deduct minutes from balance: {error}");
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

        self.events.publish_session_ended(&updated.id.to_string());

        let device_channel = format!("device:{}", session.device_id);
        let payload = serde_json::json!({ "session_id": updated.id.to_string(), "device_id": session.device_id.to_string() });
        let _ = self.outbox.publish("staff", "session.ended", payload.clone(), None, None, true).await;
        let _ = self.outbox.publish(&device_channel, "session.ended", payload, None, None, false).await;

        Ok(updated)
    }
}

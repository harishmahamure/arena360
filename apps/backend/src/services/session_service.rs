use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    CreateSessionDto, Device, EndSessionDto, SessionFilterDto, UpdateDeviceStatusDto, UsageSession,
    UsageSessionResponse, SESSION_END_REASONS,
};
use crate::realtime::OutboxService;
use crate::repositories::SessionRepository;
use crate::services::{BalanceService, DeviceService, EventService};

/// Result of starting (or resuming) a kiosk session for a player.
pub struct KioskSessionStart {
    pub session: UsageSession,
    pub balance_id: Uuid,
    pub remaining_minutes: i32,
    pub resumed: bool,
}

pub struct SessionService {
    repo: SessionRepository,
    devices: DeviceService,
    balances: Arc<BalanceService>,
    events: EventService,
    outbox: OutboxService,
}

fn elapsed_minutes_since(start_time: DateTime<Utc>) -> i32 {
    elapsed_minutes_between(start_time, Utc::now())
}

fn elapsed_minutes_between(start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> i32 {
    let duration_ms = end_time
        .signed_duration_since(start_time)
        .num_milliseconds();
    (((duration_ms as f64) / (1000.0 * 60.0)).ceil() as i32).max(0)
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
        let active = self.repo.find_active_by_balance(dto.balance_id).await?;
        if !active.is_empty() {
            return Err(AppError::Conflict(format!(
                "Balance already has an active session (ID: {})",
                active[0].id
            )));
        }

        let start_time = dto.start_time.unwrap_or_else(Utc::now);
        let validation = self
            .balances
            .validate_access(dto.balance_id, None, Some(start_time))
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
        let payload = serde_json::json!({
            "sessionId": session.id.to_string(),
            "deviceId": dto.device_id.to_string(),
        });
        let _ = self
            .outbox
            .publish(
                "staff",
                "session.started",
                payload.clone(),
                None,
                None,
                true,
            )
            .await;
        let _ = self
            .outbox
            .publish(
                &device_channel,
                "session.started",
                payload,
                None,
                None,
                false,
            )
            .await;

        Ok(session)
    }

    /// Start (or resume) a kiosk session for an authenticated player on a
    /// registered device. Enforces the global single-session rule (ADR-0017):
    /// a player open on another device is rejected; open on the same device
    /// resumes without creating a duplicate. Binds to the system kiosk shift.
    pub async fn start_for_player(
        &self,
        player_id: Uuid,
        device: &Device,
        balance_id: Option<Uuid>,
    ) -> Result<KioskSessionStart, AppError> {
        if let Some(open) = self.repo.find_open_session_for_player(player_id).await? {
            if open.device_id != device.id {
                return Err(AppError::conflict_code(
                    "PLAYER_ALREADY_IN_SESSION",
                    Some(serde_json::json!({
                        "deviceId": open.device_id.to_string(),
                        "deviceName": open.device_name,
                        "sessionId": open.session_id.to_string(),
                        "sessionStartTime": open.start_time.to_rfc3339(),
                    })),
                ));
            }
            let session = self
                .repo
                .find_by_id(open.session_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Open session vanished".to_string()))?;
            return Ok(KioskSessionStart {
                session,
                balance_id: open.balance_id,
                remaining_minutes: open.remaining_minutes,
                resumed: true,
            });
        }

        let balance = match balance_id {
            Some(id) => {
                let raw = self.balances.get_raw(id).await?;
                if raw.player_id != player_id {
                    return Err(AppError::Forbidden(
                        "Balance does not belong to this player".to_string(),
                    ));
                }
                let validation = BalanceService::validate_balance(&raw, Some(device), None);
                if !validation.valid {
                    return Err(BalanceService::validation_to_app_error(validation));
                }
                raw
            }
            None => {
                self.balances
                    .require_usable_for_device(player_id, device)
                    .await?
            }
        };

        let shift_id = self.repo.find_or_create_system_kiosk_shift().await?;

        let session = self
            .start(
                CreateSessionDto {
                    balance_id: balance.id,
                    device_id: device.id,
                    shift_id,
                    start_time: None,
                },
                None,
            )
            .await?;

        Ok(KioskSessionStart {
            session,
            balance_id: balance.id,
            remaining_minutes: balance.remaining_minutes,
            resumed: false,
        })
    }

    /// The player's current open session (if any), with fresh remaining
    /// minutes. Used by the kiosk HUD poller to resync the countdown.
    pub async fn open_session_for_player(
        &self,
        player_id: Uuid,
    ) -> Result<Option<crate::repositories::session_repo::PlayerOpenSession>, AppError> {
        self.repo.find_open_session_for_player(player_id).await
    }

    pub async fn heartbeat_for_player(
        &self,
        session_id: Uuid,
        player_id: Uuid,
        device_id: Uuid,
    ) -> Result<KioskSessionStart, AppError> {
        let session =
            self.repo.find_by_id(session_id).await?.ok_or_else(|| {
                AppError::NotFound(format!("Session with ID {session_id} not found"))
            })?;

        if session.end_time.is_some() {
            let balance_id = session
                .balance_id
                .ok_or_else(|| AppError::BadRequest("Session has no linked balance".to_string()))?;
            let balance = self.balances.get_raw(balance_id).await?;
            return Ok(KioskSessionStart {
                session,
                balance_id,
                remaining_minutes: balance.remaining_minutes,
                resumed: true,
            });
        }

        if session.device_id != device_id {
            return Err(AppError::Forbidden(
                "Session does not belong to this device".to_string(),
            ));
        }

        let balance_id = session
            .balance_id
            .ok_or_else(|| AppError::BadRequest("Session has no linked balance".to_string()))?;
        let mut balance = self.balances.get_raw(balance_id).await?;
        if balance.player_id != player_id {
            return Err(AppError::Forbidden(
                "Session does not belong to this player".to_string(),
            ));
        }

        let elapsed_minutes = elapsed_minutes_since(session.start_time);
        let already_charged = session.time_credits_consumed.unwrap_or(0).max(0);
        let delta = (elapsed_minutes - already_charged).max(0);
        let mut updated_session = session;

        if delta > 0 {
            balance = self
                .balances
                .deduct_minutes(balance_id, delta, Some(updated_session.id))
                .await?;
            updated_session = self
                .repo
                .update_time_credits_consumed(updated_session.id, already_charged + delta)
                .await?;
            self.publish_balance_updated(&updated_session, player_id, balance.remaining_minutes)
                .await;
        }

        Ok(KioskSessionStart {
            session: updated_session,
            balance_id,
            remaining_minutes: balance.remaining_minutes,
            resumed: true,
        })
    }

    pub async fn end(
        &self,
        id: Uuid,
        dto: EndSessionDto,
        actor_id: Option<Uuid>,
    ) -> Result<UsageSession, AppError> {
        let reason = match dto.reason.as_deref() {
            Some(r) if SESSION_END_REASONS.contains(&r) => Some(r.to_string()),
            Some(other) => {
                return Err(AppError::BadRequest(format!(
                    "Invalid session end reason '{other}'"
                )))
            }
            None => None,
        };
        let session = self.get_by_id(id).await?;

        if session.end_time.is_some() {
            return Err(AppError::BadRequest(
                "Session has already been ended".to_string(),
            ));
        }

        let end_time = dto.end_time.unwrap_or_else(Utc::now);
        let duration_minutes = elapsed_minutes_between(session.start_time, end_time);

        let balance_id = session.balance_id.ok_or_else(|| {
            AppError::BadRequest("Session has no linked balance (legacy session)".to_string())
        })?;

        let previous_consumed = session.time_credits_consumed.unwrap_or(0).max(0);
        let final_consumed = dto.time_credits_consumed.unwrap_or(duration_minutes).max(0);
        let deduct_delta = (final_consumed - previous_consumed).max(0);

        let updated = self
            .repo
            .end(
                id,
                end_time,
                duration_minutes,
                Some(final_consumed),
                actor_id,
            )
            .await?;

        let mut remaining_minutes = 0;
        let mut player_id = None;
        if deduct_delta > 0 {
            match self
                .balances
                .deduct_minutes(balance_id, deduct_delta, Some(updated.id))
                .await
            {
                Ok(balance) => {
                    remaining_minutes = balance.remaining_minutes;
                    player_id = Some(balance.player_id);
                }
                Err(error) => {
                    tracing::warn!("Failed to deduct minutes from balance: {error}");
                }
            }
        } else if let Ok(balance) = self.balances.get_raw(balance_id).await {
            remaining_minutes = balance.remaining_minutes;
            player_id = Some(balance.player_id);
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
        let mut payload = serde_json::json!({
            "sessionId": updated.id.to_string(),
            "deviceId": session.device_id.to_string(),
            "remainingMinutes": remaining_minutes,
        });
        if let Some(reason) = &reason {
            payload["reason"] = serde_json::Value::String(reason.clone());
        }
        if let Some(player_id) = player_id {
            payload["playerId"] = serde_json::Value::String(player_id.to_string());
        }
        let _ = self
            .outbox
            .publish("staff", "session.ended", payload.clone(), None, None, true)
            .await;
        let _ = self
            .outbox
            .publish(&device_channel, "session.ended", payload, None, None, false)
            .await;
        if let Some(player_id) = player_id {
            let user_channel = format!("user:{player_id}");
            let payload = serde_json::json!({
                "sessionId": updated.id.to_string(),
                "deviceId": session.device_id.to_string(),
                "playerId": player_id.to_string(),
                "reason": reason,
                "remainingMinutes": remaining_minutes,
            });
            let _ = self
                .outbox
                .publish(
                    &user_channel,
                    "session.ended",
                    payload,
                    None,
                    Some(player_id),
                    false,
                )
                .await;
        }

        Ok(updated)
    }

    async fn publish_balance_updated(
        &self,
        session: &UsageSession,
        player_id: Uuid,
        remaining_minutes: i32,
    ) {
        let Some(balance_id) = session.balance_id else {
            return;
        };
        let payload = serde_json::json!({
            "balanceId": balance_id.to_string(),
            "remainingMinutes": remaining_minutes,
            "playerId": player_id.to_string(),
            "sessionId": session.id.to_string(),
            "deviceId": session.device_id.to_string(),
        });
        let device_channel = format!("device:{}", session.device_id);
        let user_channel = format!("user:{player_id}");
        let _ = self
            .outbox
            .publish(
                &device_channel,
                "balance.updated",
                payload.clone(),
                None,
                None,
                false,
            )
            .await;
        let _ = self
            .outbox
            .publish(
                &user_channel,
                "balance.updated",
                payload,
                None,
                Some(player_id),
                false,
            )
            .await;
    }
}

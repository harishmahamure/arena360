use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::deduction_profile::DeductionProfile;
use crate::models::{
    CreateSessionDto, Device, EndSessionDto, PlayerPlanBalance, SessionFilterDto,
    UpdateDeviceStatusDto, UsageSession, UsageSessionResponse, SESSION_END_REASONS,
};
use crate::realtime::OutboxService;
use crate::repositories::SessionRepository;
use crate::services::deduction_profile::{
    wall_minutes_between, weighted_minutes_between,
};
use crate::services::{BalanceService, DeviceService, EventService};

/// Result of starting (or resuming) a kiosk session for a player.
pub struct KioskSessionStart {
    pub session: UsageSession,
    pub balance_id: Uuid,
    pub remaining_minutes: i32,
    pub resumed: bool,
    pub deduction_profile: Option<Value>,
    pub time_credits_consumed: f64,
    pub cafe_timezone: String,
}

pub struct SessionService {
    repo: SessionRepository,
    devices: DeviceService,
    balances: Arc<BalanceService>,
    events: EventService,
    outbox: OutboxService,
    cafe_timezone: String,
}

fn elapsed_minutes_between(start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> i32 {
    wall_minutes_between(start_time, end_time).ceil() as i32
}

fn parse_balance_profile(balance: &PlayerPlanBalance) -> Option<DeductionProfile> {
    let value = balance.deduction_profile.as_ref()?;
    serde_json::from_value(value.clone()).ok()
}

fn weighted_consumption(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    profile: Option<&DeductionProfile>,
    cafe_tz: &str,
) -> f64 {
    match profile {
        Some(p) => weighted_minutes_between(start, end, p, cafe_tz),
        None => wall_minutes_between(start, end),
    }
}

fn charged_wallet_minutes(session: &UsageSession) -> i32 {
    session.time_credits_consumed.unwrap_or(0).max(0)
}

/// Project wallet minutes left for an open session (poll/login display; does not deduct).
pub fn effective_remaining_for_session(
    balance: &PlayerPlanBalance,
    session: &UsageSession,
    cafe_tz: &str,
) -> i32 {
    let profile = parse_balance_profile(balance);
    let total = weighted_consumption(session.start_time, Utc::now(), profile.as_ref(), cafe_tz);
    let owed = (total.ceil() as i32 - charged_wallet_minutes(session)).max(0);
    (balance.remaining_minutes - owed).max(0)
}

/// Remaining play time for an open session without dynamic profile (legacy tests).
pub fn effective_remaining_minutes(balance_remaining: i32, start_time: DateTime<Utc>) -> i32 {
    let elapsed = wall_minutes_between(start_time, Utc::now()).ceil() as i32;
    (balance_remaining - elapsed).max(0)
}

impl SessionService {
    pub fn new(
        pool: PgPool,
        devices: DeviceService,
        balances: Arc<BalanceService>,
        events: EventService,
        outbox: OutboxService,
        cafe_timezone: String,
    ) -> Self {
        Self {
            repo: SessionRepository::new(pool),
            devices,
            balances,
            events,
            outbox,
            cafe_timezone,
        }
    }

    fn kiosk_session_start(
        &self,
        session: UsageSession,
        balance: &PlayerPlanBalance,
        balance_id: Uuid,
        remaining_minutes: i32,
        resumed: bool,
    ) -> KioskSessionStart {
        KioskSessionStart {
            time_credits_consumed: charged_wallet_minutes(&session) as f64,
            deduction_profile: balance.deduction_profile.clone(),
            cafe_timezone: self.cafe_timezone.clone(),
            session,
            balance_id,
            remaining_minutes,
            resumed,
        }
    }

    async fn charge_session_delta(
        &self,
        session: &UsageSession,
        balance: &PlayerPlanBalance,
        end: DateTime<Utc>,
    ) -> Result<(i32, PlayerPlanBalance), AppError> {
        let profile = parse_balance_profile(balance);
        let total = weighted_consumption(
            session.start_time,
            end,
            profile.as_ref(),
            &self.cafe_timezone,
        )
        .ceil() as i32;
        let charged = charged_wallet_minutes(session);
        let delta = (total - charged).max(0);
        if delta == 0 {
            return Ok((total, balance.clone()));
        }
        let balance_id = session
            .balance_id
            .ok_or_else(|| AppError::BadRequest("Session has no linked balance".to_string()))?;
        let updated = self
            .balances
            .deduct_minutes(balance_id, delta, Some(session.id))
            .await?;
        self.repo
            .update_time_credits_consumed(session.id, total)
            .await?;
        Ok((total, updated))
    }

    async fn publish_balance_updated(
        &self,
        player_id: Uuid,
        device_id: Uuid,
        session_id: Uuid,
        balance: &PlayerPlanBalance,
    ) {
        let payload = serde_json::json!({
            "balanceId": balance.id.to_string(),
            "remainingMinutes": balance.remaining_minutes,
            "playerId": player_id.to_string(),
            "sessionId": session_id.to_string(),
        });
        let device_channel = format!("device:{device_id}");
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

        let balance = self.balances.get_raw(dto.balance_id).await?;
        let remaining = effective_remaining_for_session(
            &balance,
            &session,
            &self.cafe_timezone,
        );
        let device_channel = format!("device:{}", dto.device_id);
        let payload = serde_json::json!({
            "sessionId": session.id.to_string(),
            "deviceId": dto.device_id.to_string(),
            "playerId": balance.player_id.to_string(),
            "balanceId": dto.balance_id.to_string(),
            "startTime": session.start_time.to_rfc3339(),
            "remainingMinutes": remaining as f64,
            "deductionProfile": balance.deduction_profile,
            "cafeTimezone": self.cafe_timezone,
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
            let balance = self.balances.get_raw(open.balance_id).await?;
            let remaining = effective_remaining_for_session(&balance, &session, &self.cafe_timezone);
            return Ok(self.kiosk_session_start(
                session,
                &balance,
                open.balance_id,
                remaining,
                true,
            ));
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

        Ok(self.kiosk_session_start(
            session,
            &balance,
            balance.id,
            balance.remaining_minutes,
            false,
        ))
    }

    /// Kiosk poll payload with deduction profile for HUD time-speeding.
    pub async fn open_kiosk_session_for_player(
        &self,
        player_id: Uuid,
    ) -> Result<Option<KioskSessionStart>, AppError> {
        let Some(open) = self.repo.find_open_session_for_player(player_id).await? else {
            return Ok(None);
        };
        let session = self
            .repo
            .find_by_id(open.session_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Open session vanished".to_string()))?;
        let balance = self.balances.get_raw(open.balance_id).await?;
        let remaining = effective_remaining_for_session(&balance, &session, &self.cafe_timezone);
        if remaining <= 0 {
            self.auto_end_expired(open.session_id).await?;
            return Ok(None);
        }
        Ok(Some(self.kiosk_session_start(
            session,
            &balance,
            open.balance_id,
            remaining,
            true,
        )))
    }

    /// The player's current open session (if any), with fresh remaining
    /// minutes. Used by the kiosk HUD poller to resync the countdown.
    pub async fn open_session_for_player(
        &self,
        player_id: Uuid,
    ) -> Result<Option<crate::repositories::session_repo::PlayerOpenSession>, AppError> {
        let Some(mut open) = self.repo.find_open_session_for_player(player_id).await? else {
            return Ok(None);
        };
        let balance = self.balances.get_raw(open.balance_id).await?;
        let session = self
            .repo
            .find_by_id(open.session_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Open session vanished".to_string()))?;
        open.remaining_minutes =
            effective_remaining_for_session(&balance, &session, &self.cafe_timezone);
        if open.remaining_minutes <= 0 {
            self.auto_end_expired(open.session_id).await?;
            return Ok(None);
        }
        Ok(Some(open))
    }

    /// Close an expired open session with `reason = auto` and persist `endTime`.
    async fn auto_end_expired(&self, session_id: Uuid) -> Result<(), AppError> {
        if let Some(session) = self.repo.find_by_id(session_id).await? {
            if session.end_time.is_none() {
                self.end(
                    session_id,
                    EndSessionDto {
                        end_time: Some(Utc::now()),
                        time_credits_consumed: None,
                        staff_totp: None,
                        reason: Some("auto".to_string()),
                    },
                    None,
                )
                .await?;
            }
        }
        Err(AppError::NotFound(format!(
            "Session with ID {session_id} has ended"
        )))
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
            return Err(AppError::NotFound(format!(
                "Session with ID {session_id} has ended"
            )));
        }

        if session.device_id != device_id {
            return Err(AppError::Forbidden(
                "Session does not belong to this device".to_string(),
            ));
        }

        let balance_id = session
            .balance_id
            .ok_or_else(|| AppError::BadRequest("Session has no linked balance".to_string()))?;
        let balance = self.balances.get_raw(balance_id).await?;
        if balance.player_id != player_id {
            return Err(AppError::Forbidden(
                "Session does not belong to this player".to_string(),
            ));
        }

        let (_, updated_balance) = self
            .charge_session_delta(&session, &balance, Utc::now())
            .await?;
        let session = self
            .repo
            .find_by_id(session_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Session with ID {session_id} not found")))?;
        let remaining = updated_balance.remaining_minutes;

        if remaining <= 0 {
            self.auto_end_expired(session_id).await?;
        }

        self.publish_balance_updated(
            player_id,
            device_id,
            session_id,
            &updated_balance,
        )
        .await;

        Ok(self.kiosk_session_start(
            session,
            &updated_balance,
            balance_id,
            remaining,
            true,
        ))
    }

    /// Close a session and charge the final wallet delta from server time.
    ///
    /// For `auto`, staff `force`, kiosk `voluntary`, and staff PATCH end, callers
    /// must leave `time_credits_consumed` unset so weighted minutes are computed
    /// via `charge_session_delta`. Only `offline_reconcile` may supply a client
    /// total after connectivity loss.
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

        let raw_session = self
            .repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Session with ID {id} not found")))?;
        let balance = self.balances.get_raw(balance_id).await?;

        let (time_used, final_balance) = if dto.time_credits_consumed.is_some() {
            let time_used = dto.time_credits_consumed.unwrap_or(0).max(0);
            let updated_balance = if time_used > 0 {
                self.balances
                    .deduct_minutes(balance_id, time_used, Some(id))
                    .await?
            } else {
                balance
            };
            (time_used, updated_balance)
        } else {
            let (total, updated_balance) = self
                .charge_session_delta(&raw_session, &balance, end_time)
                .await?;
            (total, updated_balance)
        };

        let updated = self
            .repo
            .end(id, end_time, duration_minutes, Some(time_used), actor_id)
            .await?;

        let remaining_minutes = final_balance.remaining_minutes;
        let player_id = Some(final_balance.player_id);

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
            "endTime": updated.end_time.map(|t| t.to_rfc3339()),
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
                "endTime": updated.end_time.map(|t| t.to_rfc3339()),
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

    pub async fn open_tv_session_for_device(
        &self,
        device: &Device,
    ) -> Result<Option<crate::dto::TvSessionResponseDto>, AppError> {
        crate::validation::require_playstation_device_type(Some(device.device_type.clone()))?;

        let Some(session) = self
            .repo
            .find_open_session_for_device(device.id)
            .await?
        else {
            return Ok(None);
        };

        let balance_id = session.balance_id.ok_or_else(|| {
            AppError::BadRequest("Session has no linked balance".to_string())
        })?;
        let balance = self.balances.get_raw(balance_id).await?;
        let remaining =
            effective_remaining_for_session(&balance, &session, &self.cafe_timezone);
        let deduction_profile = balance
            .deduction_profile
            .as_ref()
            .and_then(|value| serde_json::from_value::<DeductionProfile>(value.clone()).ok());

        Ok(Some(crate::dto::TvSessionResponseDto {
            sessionId: session.id.to_string(),
            balanceId: balance_id.to_string(),
            deviceId: device.id.to_string(),
            startTime: session.start_time.to_rfc3339(),
            remainingMinutes: remaining as f64,
            playerUsername: None,
            deductionProfile: deduction_profile,
            cafeTimezone: self.cafe_timezone.clone(),
        }))
    }

    pub async fn end_tv_session_for_device(
        &self,
        device: &Device,
        session_id: Uuid,
        reason: Option<String>,
    ) -> Result<crate::dto::TvSessionResponseDto, AppError> {
        crate::validation::require_playstation_device_type(Some(device.device_type.clone()))?;

        let session = self.get_by_id(session_id).await?;
        if session.device_id != device.id {
            return Err(AppError::Forbidden(
                "Session does not belong to this device".to_string(),
            ));
        }

        if session.end_time.is_some() {
            let balance_id = session.balance_id.unwrap_or_default();
            let (remaining, deduction_profile) = if balance_id != Uuid::nil() {
                let balance = self.balances.get_raw(balance_id).await?;
                (
                    balance.remaining_minutes,
                    balance.deduction_profile.as_ref().and_then(|value| {
                        serde_json::from_value::<DeductionProfile>(value.clone()).ok()
                    }),
                )
            } else {
                (0, None)
            };
            return Ok(crate::dto::TvSessionResponseDto {
                sessionId: session.id.to_string(),
                balanceId: session
                    .balance_id
                    .map(|b| b.to_string())
                    .unwrap_or_default(),
                deviceId: device.id.to_string(),
                startTime: session.start_time.to_rfc3339(),
                remainingMinutes: remaining as f64,
                playerUsername: None,
                deductionProfile: deduction_profile,
                cafeTimezone: self.cafe_timezone.clone(),
            });
        }

        let end_reason = reason.unwrap_or_else(|| "auto".to_string());
        let ended = self
            .end(
                session_id,
                EndSessionDto {
                    end_time: None,
                    time_credits_consumed: None,
                    staff_totp: None,
                    reason: Some(end_reason),
                },
                None,
            )
            .await?;

        let balance_id = ended.balance_id.unwrap_or_default();
        let (remaining, deduction_profile) = if balance_id != Uuid::nil() {
            let balance = self.balances.get_raw(balance_id).await?;
            (
                balance.remaining_minutes,
                balance.deduction_profile.as_ref().and_then(|value| {
                    serde_json::from_value::<DeductionProfile>(value.clone()).ok()
                }),
            )
        } else {
            (0, None)
        };

        Ok(crate::dto::TvSessionResponseDto {
            sessionId: ended.id.to_string(),
            balanceId: balance_id.to_string(),
            deviceId: device.id.to_string(),
            startTime: ended.start_time.to_rfc3339(),
            remainingMinutes: remaining as f64,
            playerUsername: None,
            deductionProfile: deduction_profile,
            cafeTimezone: self.cafe_timezone.clone(),
        })
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_remaining_for_session_uses_weighted_consumption() {
        let start = Utc::now() - chrono::Duration::minutes(30);
        let balance = PlayerPlanBalance {
            id: Uuid::new_v4(),
            player_id: Uuid::new_v4(),
            device_type: None,
            device_sub_type: None,
            kind: "time".to_string(),
            remaining_minutes: 300,
            expiry_date: Utc::now() + chrono::Duration::days(30),
            window_start: None,
            window_end: None,
            status: "active".to_string(),
            source_plan_id: None,
            allowed_days: None,
            allowed_months: None,
            deduction_profile: Some(serde_json::json!({
                "peakWindowStart": "18:00:00",
                "peakWindowEnd": "23:00:00",
                "peakRatio": 1.5,
                "lowWindowStart": "07:00:00",
                "lowWindowEnd": "11:00:00",
                "lowRatio": 0.8
            })),
            created_by: None,
            updated_by: None,
            created_at: start,
            updated_at: start,
            deleted_at: None,
        };
        let session = UsageSession {
            id: Uuid::new_v4(),
            balance_id: Some(balance.id),
            device_id: Uuid::new_v4(),
            shift_id: None,
            start_time: start,
            end_time: None,
            duration_minutes: None,
            time_credits_consumed: Some(0),
            created_by: None,
            updated_by: None,
            created_at: start,
            updated_at: start,
            deleted_at: None,
        };
        let remaining = effective_remaining_for_session(&balance, &session, "Asia/Kolkata");
        assert!(
            remaining < 300,
            "open session should project less wallet time after 30 wall minutes, got {remaining}"
        );
    }

    #[test]
    fn effective_remaining_subtracts_elapsed_minutes() {
        let start = Utc::now() - chrono::Duration::minutes(15);
        assert_eq!(effective_remaining_minutes(60, start), 45);
    }

    #[test]
    fn effective_remaining_never_negative() {
        let start = Utc::now() - chrono::Duration::minutes(30);
        assert_eq!(effective_remaining_minutes(5, start), 0);
    }
}

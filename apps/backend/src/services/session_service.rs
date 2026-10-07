use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, keys, CacheService};
use crate::error::AppError;
use crate::models::deduction_profile::DeductionProfile;
use crate::models::{
    CreateSessionDto, Device, EndSessionDto, PlayerPlanBalance, SessionFilterDto, UsageSession,
    UsageSessionResponse, SESSION_END_REASONS,
};
use crate::repositories::{
    TenantBalanceRepository, TenantDeviceRepository, TenantSessionRepository,
};
use crate::services::deduction_profile::{wall_minutes_between, weighted_minutes_between};
use crate::services::{BalanceService, ConfigService, DeviceService, EventService};
use crate::tenancy::TenantDb;

/// Result of starting (or resuming) a kiosk session for a player.
pub struct KioskSessionStart {
    pub session: UsageSession,
    pub balance_id: Uuid,
    /// Raw wallet minutes from `player_plan_balances.remainingMinutes`.
    pub wallet_balance_minutes: i32,
    /// Server-computed effective display remaining for legacy clients.
    pub remaining_minutes: i32,
    pub resumed: bool,
    pub deduction_profile: Option<Value>,
    pub time_credits_consumed: f64,
    pub cafe_timezone: String,
    pub expiry_date: DateTime<Utc>,
}

pub struct SessionService {
    pricing: crate::services::PricingPolicyService,
    settings: Arc<ConfigService>,
    devices: DeviceService,
    balances: Arc<BalanceService>,
    events: EventService,
    cache: Arc<dyn CacheService>,
}

fn elapsed_minutes_between(start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> i32 {
    wall_minutes_between(start_time, end_time).ceil() as i32
}

pub fn session_profile_value<'a>(
    balance: &'a PlayerPlanBalance,
    session: &'a UsageSession,
) -> Option<&'a Value> {
    session
        .deduction_profile_snapshot
        .as_ref()
        .or(balance.deduction_profile.as_ref())
}

fn parse_session_profile(
    balance: &PlayerPlanBalance,
    session: &UsageSession,
) -> Option<DeductionProfile> {
    let value = session_profile_value(balance, session)?;
    serde_json::from_value(value.clone()).ok()
}

fn effective_session_timezone(
    balance: &PlayerPlanBalance,
    session: &UsageSession,
    fallback: &str,
) -> String {
    session
        .deduction_profile_snapshot
        .as_ref()
        .and_then(|value| value.get("policyTimezone"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            balance
                .deduction_profile
                .as_ref()
                .and_then(|value| value.get("policyTimezone"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| fallback.to_owned())
}

fn snapshot_timezone(session: &UsageSession, fallback: &str) -> String {
    session
        .deduction_profile_snapshot
        .as_ref()
        .and_then(|value| value.get("policyTimezone"))
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_owned()
}

fn capture_deduction_profile(
    value: Option<&Value>,
    rules: Vec<crate::models::PricingRule>,
    device_type: &str,
) -> Result<Value, AppError> {
    let mut profile = match value {
        Some(value) => {
            serde_json::from_value::<DeductionProfile>(value.clone()).map_err(|error| {
                AppError::Internal(format!("Invalid plan deduction profile: {error}"))
            })?
        }
        None => DeductionProfile::normal(),
    };
    profile.policy_rules = rules
        .into_iter()
        .filter(|rule| {
            rule.target == crate::models::PricingTarget::Deduction
                && (rule.device_types.is_empty()
                    || rule
                        .device_types
                        .iter()
                        .any(|t| t.eq_ignore_ascii_case(device_type)))
        })
        .map(|mut rule| {
            rule.device_types.clear();
            rule
        })
        .collect();
    serde_json::to_value(profile).map_err(|error| AppError::Internal(error.to_string()))
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

/// Floor minutes from `now` until `expiry` (0 when already expired).
pub fn minutes_until_expiry(expiry: DateTime<Utc>, now: DateTime<Utc>) -> i32 {
    ((expiry - now).num_seconds().max(0) as f64 / 60.0).floor() as i32
}

/// Project wallet minutes left for an open session (poll/login display; does not deduct).
pub fn effective_remaining_for_session(
    balance: &PlayerPlanBalance,
    session: &UsageSession,
    cafe_tz: &str,
) -> i32 {
    let profile = parse_session_profile(balance, session);
    let timezone = effective_session_timezone(balance, session, cafe_tz);
    let total = weighted_consumption(session.start_time, Utc::now(), profile.as_ref(), &timezone);
    let owed = (total.ceil() as i32 - charged_wallet_minutes(session)).max(0);
    (balance.remaining_minutes - owed).max(0)
}

/// Display remaining capped by plan expiry: min(walletRemaining, minutesUntilExpiry).
pub fn display_remaining_for_session(
    balance: &PlayerPlanBalance,
    session: &UsageSession,
    cafe_tz: &str,
) -> i32 {
    let now = Utc::now();
    if now > balance.expiry_date {
        return 0;
    }
    let wallet = effective_remaining_for_session(balance, session, cafe_tz);
    wallet.min(minutes_until_expiry(balance.expiry_date, now))
}

/// Remaining play time for an open session without dynamic profile (legacy tests).
pub fn effective_remaining_minutes(balance_remaining: i32, start_time: DateTime<Utc>) -> i32 {
    let elapsed = wall_minutes_between(start_time, Utc::now()).ceil() as i32;
    (balance_remaining - elapsed).max(0)
}

fn with_cafe_timezone(mut session: UsageSessionResponse, tz: &str) -> UsageSessionResponse {
    session.cafe_timezone = tz.to_string();
    session
}

impl SessionService {
    pub fn new(
        devices: DeviceService,
        balances: Arc<BalanceService>,
        events: EventService,
        settings: Arc<ConfigService>,
        pricing: crate::services::PricingPolicyService,
        cache: Arc<dyn CacheService>,
    ) -> Self {
        Self {
            devices,
            balances,
            events,
            settings,
            pricing,
            cache,
        }
    }

    async fn invalidate_sessions_list_cache(&self) -> Result<(), AppError> {
        self.cache
            .invalidate_prefix(keys::SESSIONS_LIST_PREFIX)
            .await
    }

    async fn invalidate_session(&self, session: &UsageSession) -> Result<(), AppError> {
        let keys_to_drop = vec![
            keys::session_enriched(&session.id),
            keys::session_device(&session.device_id),
        ];
        cache::invalidate(&*self.cache, &keys_to_drop).await?;
        self.invalidate_sessions_list_cache().await
    }

    async fn invalidate_stats_cache(&self) {
        let _ = cache::invalidate_stats(&*self.cache).await;
    }

    fn kiosk_session_start_tenant(
        &self,
        session: UsageSession,
        balance: &PlayerPlanBalance,
        resumed: bool,
        fallback_timezone: String,
    ) -> KioskSessionStart {
        let timezone = effective_session_timezone(balance, &session, &fallback_timezone);
        KioskSessionStart {
            wallet_balance_minutes: balance.remaining_minutes,
            remaining_minutes: display_remaining_for_session(balance, &session, &timezone),
            time_credits_consumed: charged_wallet_minutes(&session) as f64,
            deduction_profile: session_profile_value(balance, &session).cloned(),
            cafe_timezone: timezone,
            expiry_date: balance.expiry_date,
            balance_id: balance.id,
            session,
            resumed,
        }
    }

    async fn tenant_timezone(
        &self,
        db: Arc<TenantDb>,
        location_id: Uuid,
    ) -> Result<String, AppError> {
        Ok(self
            .settings
            .venue_pricing_context_tenant(db.clone(), db.tenant_id(), Some(location_id))
            .await?
            .0)
    }

    async fn tenant_profile(
        &self,
        db: Arc<TenantDb>,
        balance: &PlayerPlanBalance,
        device: &Device,
    ) -> Result<(Value, String), AppError> {
        let rules = self
            .pricing
            .active_rules_tenant(
                db.clone(),
                db.tenant_id(),
                Some(device.location_id),
                crate::models::PricingTarget::Deduction,
            )
            .await?;
        let mut snapshot = capture_deduction_profile(
            balance.deduction_profile.as_ref(),
            rules,
            &device.device_type,
        )?;
        let timezone = self.tenant_timezone(db, device.location_id).await?;
        snapshot["policyTimezone"] = Value::String(timezone.clone());
        Ok((snapshot, timezone))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn start_tenant(
        &self,
        db: Arc<TenantDb>,
        dto: CreateSessionDto,
        player_id: Uuid,
        actor_id: Option<Uuid>,
    ) -> Result<UsageSession, AppError> {
        let balance = TenantBalanceRepository::new(db.clone())
            .find_by_id(dto.balance_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Balance with ID {} not found", dto.balance_id))
            })?;
        let device = TenantDeviceRepository::new(db.clone())
            .find_by_id(dto.device_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Device with ID {} not found", dto.device_id))
            })?;
        let validation = BalanceService::validate_balance(&balance, Some(&device), dto.start_time);
        if !validation.valid {
            return Err(BalanceService::validation_to_app_error_for_balance(
                &balance, validation,
            ));
        }
        let (snapshot, _) = self.tenant_profile(db.clone(), &balance, &device).await?;
        // API-0030 owns shift projection. This remains connection-ready, but a
        // caller may supply only a shift already present in this tenant database.
        let mutation = TenantSessionRepository::new(db.clone())
            .start(
                player_id,
                balance.id,
                device.id,
                device.location_id,
                dto.shift_id,
                dto.start_time.unwrap_or_else(Utc::now),
                actor_id,
                snapshot,
            )
            .await?;
        self.after_tenant_session_mutation(db.clone(), &mutation.session, &mutation.balance)
            .await;
        self.publish_tenant_device_status(db, device.id).await;
        Ok(mutation.session)
    }

    pub async fn start_for_player_tenant(
        &self,
        db: Arc<TenantDb>,
        player_id: Uuid,
        device: &Device,
        balance_id: Option<Uuid>,
    ) -> Result<KioskSessionStart, AppError> {
        let sessions = TenantSessionRepository::new(db.clone());
        if let Some(open) = sessions.find_open_for_player(player_id).await? {
            if open.device_id != device.id {
                return Err(AppError::conflict_code(
                    "PLAYER_ALREADY_IN_SESSION",
                    Some(
                        json!({"deviceId":open.device_id,"deviceName":open.device_name,
                        "sessionId":open.session_id,"sessionStartTime":crate::time::utc_timestamp(&open.start_time)}),
                    ),
                ));
            }
            let session = sessions
                .find_by_id(open.session_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Open session vanished".into()))?;
            let balance = TenantBalanceRepository::new(db.clone())
                .find_by_id(open.balance_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Open balance vanished".into()))?;
            let fallback = self.tenant_timezone(db, device.location_id).await?;
            return Ok(self.kiosk_session_start_tenant(session, &balance, true, fallback));
        }
        let balance = if let Some(id) = balance_id {
            let balance = TenantBalanceRepository::new(db.clone())
                .find_by_id(id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Balance with ID {id} not found")))?;
            if balance.player_id != player_id {
                return Err(AppError::Forbidden(
                    "Balance does not belong to this player".into(),
                ));
            }
            let validation = BalanceService::validate_balance(&balance, Some(device), None);
            if !validation.valid {
                return Err(BalanceService::validation_to_app_error_for_balance(
                    &balance, validation,
                ));
            }
            balance
        } else {
            self.require_usable_balance_tenant(db.clone(), player_id, device)
                .await?
        };
        let session = self
            .start_tenant(
                db.clone(),
                CreateSessionDto {
                    balance_id: balance.id,
                    device_id: device.id,
                    shift_id: None,
                    start_time: None,
                },
                player_id,
                None,
            )
            .await?;
        let balance = TenantBalanceRepository::new(db.clone())
            .find_by_id(balance.id)
            .await?
            .ok_or_else(|| AppError::Internal("Balance disappeared".into()))?;
        let fallback = self.tenant_timezone(db, device.location_id).await?;
        Ok(self.kiosk_session_start_tenant(session, &balance, false, fallback))
    }

    async fn require_usable_balance_tenant(
        &self,
        db: Arc<TenantDb>,
        player_id: Uuid,
        device: &Device,
    ) -> Result<PlayerPlanBalance, AppError> {
        let rows = TenantBalanceRepository::new(db.clone())
            .list(&crate::models::BalanceFilterDto {
                player_id: Some(player_id),
                status: Some("active".into()),
                usable_only: Some(true),
                limit: Some(100),
                ..Default::default()
            })
            .await?;
        let mut best = None;
        for row in rows.data {
            let balance = TenantBalanceRepository::new(db.clone())
                .find_by_id(row.id)
                .await?
                .ok_or_else(|| AppError::Internal("Balance disappeared".into()))?;
            if BalanceService::device_scope_matches(&balance, device)
                && BalanceService::validate_balance(&balance, Some(device), None).valid
                && best.as_ref().is_none_or(|current: &PlayerPlanBalance| {
                    balance.remaining_minutes > current.remaining_minutes
                })
            {
                best = Some(balance);
            }
        }
        best.ok_or_else(|| AppError::forbidden_code("PLAN_NOT_ACTIVATED"))
    }

    pub async fn get_by_id_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
        timezone: String,
    ) -> Result<UsageSessionResponse, AppError> {
        let repo = TenantSessionRepository::new(db);
        let raw = repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Session with ID {id} not found")))?;
        repo.find_enriched_by_id(id)
            .await?
            .map(|session| with_cafe_timezone(session, &snapshot_timezone(&raw, &timezone)))
            .ok_or_else(|| AppError::NotFound(format!("Session with ID {id} not found")))
    }

    pub async fn list_tenant(
        &self,
        db: Arc<TenantDb>,
        filters: SessionFilterDto,
        locations: Vec<Uuid>,
        timezone: String,
    ) -> Result<crate::dto::PaginationResult<UsageSessionResponse>, AppError> {
        let repo = TenantSessionRepository::new(db);
        let mut result = repo.list(&filters, &locations).await?;
        for session in &mut result.data {
            let raw = repo.find_by_id(session.id).await?.ok_or_else(|| {
                AppError::NotFound(format!("Session with ID {} not found", session.id))
            })?;
            session.cafe_timezone = snapshot_timezone(&raw, &timezone);
        }
        Ok(result)
    }

    pub async fn heartbeat_for_player_tenant(
        &self,
        db: Arc<TenantDb>,
        session_id: Uuid,
        player_id: Uuid,
        device_id: Uuid,
    ) -> Result<KioskSessionStart, AppError> {
        let repo = TenantSessionRepository::new(db.clone());
        let session = repo
            .find_by_id(session_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Session with ID {session_id} not found")))?;
        if session.device_id != device_id {
            return Err(AppError::Forbidden(
                "Session does not belong to this device".into(),
            ));
        }
        let balance = TenantBalanceRepository::new(db.clone())
            .find_by_id(session.balance_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Session balance not found".into()))?;
        if balance.player_id != player_id {
            return Err(AppError::Forbidden(
                "Session does not belong to this player".into(),
            ));
        }
        let device = TenantDeviceRepository::new(db.clone())
            .find_by_id(device_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Device not found".into()))?;
        let fallback_timezone = self.tenant_timezone(db.clone(), device.location_id).await?;
        let timezone = effective_session_timezone(&balance, &session, &fallback_timezone);
        let profile = parse_session_profile(&balance, &session);
        let total = weighted_consumption(
            session.start_time,
            Utc::now(),
            profile.as_ref(),
            &timezone,
        )
        .ceil() as i32;
        let mutation = repo.charge(session_id, total, None, None).await?;
        self.after_tenant_session_mutation(db.clone(), &mutation.session, &mutation.balance)
            .await;
        if mutation.session.end_time.is_some() {
            self.publish_tenant_device_status(db, device_id).await;
            return Err(AppError::NotFound(format!(
                "Session with ID {session_id} has ended"
            )));
        }
        Ok(self.kiosk_session_start_tenant(mutation.session, &mutation.balance, true, timezone))
    }

    pub async fn open_kiosk_session_for_player_tenant(
        &self,
        db: Arc<TenantDb>,
        player_id: Uuid,
    ) -> Result<Option<KioskSessionStart>, AppError> {
        let repo = TenantSessionRepository::new(db.clone());
        let Some(open) = repo.find_open_for_player(player_id).await? else {
            return Ok(None);
        };
        let session = repo
            .find_by_id(open.session_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Open session vanished".into()))?;
        let balance = TenantBalanceRepository::new(db.clone())
            .find_by_id(open.balance_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Open balance vanished".into()))?;
        let device = TenantDeviceRepository::new(db.clone())
            .find_by_id(open.device_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Open device vanished".into()))?;
        let fallback_timezone = self.tenant_timezone(db.clone(), device.location_id).await?;
        let timezone = effective_session_timezone(&balance, &session, &fallback_timezone);
        if display_remaining_for_session(&balance, &session, &timezone) <= 0 {
            self.end_tenant(
                db,
                session.id,
                EndSessionDto {
                    reason: Some("auto".into()),
                    ..Default::default()
                },
                None,
            )
            .await?;
            return Ok(None);
        }
        Ok(Some(self.kiosk_session_start_tenant(
            session, &balance, true, timezone,
        )))
    }

    pub async fn end_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
        dto: EndSessionDto,
        actor: Option<Uuid>,
    ) -> Result<UsageSession, AppError> {
        let reason = dto.reason.unwrap_or_else(|| "voluntary".into());
        if !SESSION_END_REASONS.contains(&reason.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Invalid session end reason '{reason}'"
            )));
        }
        if dto.time_credits_consumed.is_some() && reason != "offline_reconcile" {
            return Err(AppError::BadRequest(
                "Client timeCreditsConsumed is allowed only for offline_reconcile".into(),
            ));
        }
        let repo = TenantSessionRepository::new(db.clone());
        let session = repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Session with ID {id} not found")))?;
        if session.end_time.is_some() {
            return Ok(session);
        }
        let balance = TenantBalanceRepository::new(db.clone())
            .find_by_id(session.balance_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Session balance not found".into()))?;
        let device = TenantDeviceRepository::new(db.clone())
            .find_by_id(session.device_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Device not found".into()))?;
        let fallback_timezone = self.tenant_timezone(db.clone(), device.location_id).await?;
        let timezone = effective_session_timezone(&balance, &session, &fallback_timezone);
        let end = dto.end_time.unwrap_or_else(Utc::now);
        let profile = parse_session_profile(&balance, &session);
        let total = dto.time_credits_consumed.unwrap_or_else(|| {
            weighted_consumption(session.start_time, end, profile.as_ref(), &timezone).ceil() as i32
        });
        let mutation = repo
            .charge(
                id,
                total,
                Some((
                    end,
                    elapsed_minutes_between(session.start_time, end),
                    reason.clone(),
                )),
                actor,
            )
            .await?;
        self.after_tenant_session_mutation(db.clone(), &mutation.session, &mutation.balance)
            .await;
        self.publish_tenant_device_status(db, session.device_id)
            .await;
        Ok(mutation.session)
    }

    async fn after_tenant_session_mutation(
        &self,
        db: Arc<TenantDb>,
        session: &UsageSession,
        balance: &PlayerPlanBalance,
    ) {
        if session.end_time.is_some() {
            self.events.publish_session_ended(&session.id.to_string());
        } else if session.time_credits_consumed.unwrap_or(0) == 0 {
            self.events.publish_session_started(&session.id.to_string());
        }
        let _ = self.invalidate_session(session).await;
        let _ = self
            .balances
            .sync_tenant_balance_cache_after_mutation(db, balance)
            .await;
        self.invalidate_stats_cache().await;
    }

    async fn publish_tenant_device_status(&self, db: Arc<TenantDb>, device_id: Uuid) {
        if let Ok(Some(device)) = TenantDeviceRepository::new(db).find_by_id(device_id).await {
            self.devices.after_tenant_mutation(&device).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_balances_receive_device_scoped_speed_without_changing_the_plan() {
        use chrono::TimeZone;
        let rule: crate::models::PricingRule = serde_json::from_value(serde_json::json!({ "id": "speed", "name": "speed", "target": "deduction", "priority": 100, "deviceTypes": ["PC", "PS5"], "weekdays": [], "action": { "type": "multiplier", "value": "1.25" } })).unwrap();
        let start = Utc.with_ymd_and_hms(2026, 9, 24, 18, 30, 0).unwrap();
        for device in ["PC", "PS5", "pc", "OTHER"] {
            let value = capture_deduction_profile(None, vec![rule.clone()], device).unwrap();
            let profile: DeductionProfile = serde_json::from_value(value).unwrap();
            let consumed = weighted_consumption(
                start,
                start + chrono::Duration::minutes(60),
                Some(&profile),
                "Asia/Kolkata",
            );
            assert_eq!(consumed, if device == "OTHER" { 60.0 } else { 75.0 });
        }
    }

    #[test]
    fn session_snapshot_timezone_survives_venue_timezone_change() {
        use chrono::TimeZone;

        let start = Utc.with_ymd_and_hms(2026, 9, 24, 12, 0, 0).unwrap();
        let snapshot = serde_json::json!({
            "peakWindowStart": "12:00:00",
            "peakWindowEnd": "13:00:00",
            "peakRatio": 2.0,
            "lowWindowStart": "00:00:00",
            "lowWindowEnd": "00:00:00",
            "lowRatio": 1.0,
            "policyTimezone": "UTC"
        });
        let balance = PlayerPlanBalance {
            id: Uuid::now_v7(),
            player_id: Uuid::now_v7(),
            device_type: Some("PC".into()),
            device_sub_type: None,
            kind: "time".into(),
            remaining_minutes: 300,
            expiry_date: start + chrono::Duration::days(30),
            window_start: None,
            window_end: None,
            status: "active".into(),
            source_plan_id: None,
            allowed_days: None,
            allowed_months: None,
            deduction_profile: None,
            created_by: None,
            updated_by: None,
            created_at: start,
            updated_at: start,
            deleted_at: None,
        };
        let session = UsageSession {
            id: Uuid::now_v7(),
            balance_id: balance.id,
            device_id: Uuid::now_v7(),
            shift_id: None,
            start_time: start,
            end_time: None,
            duration_minutes: None,
            time_credits_consumed: Some(0),
            wallet_minutes_at_start: Some(300),
            source_plan_id_at_start: None,
            deduction_profile_snapshot: Some(snapshot),
            created_by: None,
            updated_by: None,
            created_at: start,
            updated_at: start,
            deleted_at: None,
        };
        let timezone = effective_session_timezone(&balance, &session, "Asia/Kolkata");
        let profile = parse_session_profile(&balance, &session).unwrap();
        assert_eq!(timezone, "UTC");
        assert_eq!(
            weighted_consumption(
                start,
                start + chrono::Duration::minutes(60),
                Some(&profile),
                &timezone,
            ),
            120.0
        );
    }

    #[test]
    fn with_cafe_timezone_sets_response_field() {
        let now = Utc::now();
        let session = UsageSessionResponse {
            id: Uuid::new_v4(),
            balance_id: Uuid::new_v4(),
            device_id: Uuid::new_v4(),
            shift_id: None,
            start_time: now,
            end_time: None,
            duration_minutes: None,
            time_credits_consumed: None,
            wallet_minutes_at_start: None,
            source_plan_id_at_start: None,
            plan_at_start: None,
            created_by: None,
            updated_by: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            balance: None,
            device: None,
            cafe_timezone: String::new(),
        };
        let enriched = with_cafe_timezone(session, "Asia/Kolkata");
        assert_eq!(enriched.cafe_timezone, "Asia/Kolkata");
    }

    #[test]
    fn display_remaining_caps_by_expiry() {
        let start = Utc::now() - chrono::Duration::minutes(5);
        let balance = PlayerPlanBalance {
            id: Uuid::new_v4(),
            player_id: Uuid::new_v4(),
            device_type: None,
            device_sub_type: None,
            kind: "time".to_string(),
            remaining_minutes: 300,
            expiry_date: Utc::now() + chrono::Duration::minutes(15),
            window_start: None,
            window_end: None,
            status: "active".to_string(),
            source_plan_id: None,
            allowed_days: None,
            allowed_months: None,
            deduction_profile: None,
            created_by: None,
            updated_by: None,
            created_at: start,
            updated_at: start,
            deleted_at: None,
        };
        let session = UsageSession {
            deduction_profile_snapshot: None,
            id: Uuid::new_v4(),
            balance_id: balance.id,
            device_id: Uuid::new_v4(),
            shift_id: None,
            start_time: start,
            end_time: None,
            duration_minutes: None,
            time_credits_consumed: Some(0),
            wallet_minutes_at_start: None,
            source_plan_id_at_start: None,
            created_by: None,
            updated_by: None,
            created_at: start,
            updated_at: start,
            deleted_at: None,
        };
        let display = display_remaining_for_session(&balance, &session, "Asia/Kolkata");
        assert!(
            display <= 15,
            "expiry in 15 min should cap display remaining, got {display}"
        );
        assert!(display > 0);
    }

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
            deduction_profile_snapshot: None,
            id: Uuid::new_v4(),
            balance_id: balance.id,
            device_id: Uuid::new_v4(),
            shift_id: None,
            start_time: start,
            end_time: None,
            duration_minutes: None,
            time_credits_consumed: Some(0),
            wallet_minutes_at_start: None,
            source_plan_id_at_start: None,
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
        let remaining = effective_remaining_minutes(60, start);
        assert!(
            (44..=45).contains(&remaining),
            "clock advancement may cross the next ceil-minute boundary: {remaining}"
        );
    }

    #[test]
    fn effective_remaining_never_negative() {
        let start = Utc::now() - chrono::Duration::minutes(30);
        assert_eq!(effective_remaining_minutes(5, start), 0);
    }

    #[test]
    fn staged_tenant_heartbeat_has_no_postgres_realtime_mirror() {
        let source = include_str!("session_service.rs");
        let tenant_heartbeat = source
            .split("pub async fn heartbeat_for_player_tenant")
            .nth(1)
            .unwrap()
            .split("pub async fn open_kiosk_session_for_player_tenant")
            .next()
            .unwrap();
        assert!(!tenant_heartbeat.contains("publish_balance_updated_for_session"));
        assert!(!tenant_heartbeat.contains(".outbox.publish"));
    }
}

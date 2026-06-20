use chrono::{DateTime, Datelike, Duration, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, get_or_set, keys, CacheService};
use crate::error::AppError;
use crate::models::{
    balance_status, ledger_reason, plan_kind, BalanceFilterDto, BalanceValidationResult, Device,
    Plan, PlayerPlanBalance, PlayerPlanBalanceResponse, PurchaseBalanceDto,
};
use crate::repositories::{BalanceRepository, LedgerRepository, PlanRepository};

pub struct BalanceService {
    repo: BalanceRepository,
    ledger: LedgerRepository,
    plan_repo: PlanRepository,
    cache: Arc<dyn CacheService>,
}

fn plan_kind_from_plan(plan: &Plan) -> &'static str {
    if plan.plan_type == "weekend_special" {
        plan_kind::HAPPY_HOURS
    } else {
        plan_kind::TIME
    }
}

/// Whether an existing balance still has usable time that should stack on recharge.
fn should_carry_forward_minutes(balance: &PlayerPlanBalance, now: DateTime<Utc>) -> bool {
    balance.status == balance_status::ACTIVE && balance.expiry_date > now
}

impl BalanceService {
    pub fn new(pool: PgPool, cache: Arc<dyn CacheService>) -> Self {
        Self {
            repo: BalanceRepository::new(pool.clone()),
            ledger: LedgerRepository::new(pool.clone()),
            plan_repo: PlanRepository::new(pool),
            cache,
        }
    }

    async fn invalidate_balance(&self, balance: &PlayerPlanBalance) -> Result<(), AppError> {
        let scope = format!(
            "{}:{}:{}",
            balance.device_type.as_deref().unwrap_or("null"),
            balance.device_sub_type.as_deref().unwrap_or("null"),
            balance.kind
        );
        cache::invalidate(
            &*self.cache,
            &[keys::balance_active(&balance.player_id, &scope)],
        )
        .await
    }

    pub async fn get_raw(&self, id: Uuid) -> Result<PlayerPlanBalance, AppError> {
        let cache_key = format!("balance:raw:{id}");
        get_or_set(&*self.cache, &cache_key, keys::ttl::SESSION, || async {
            self.repo
                .find_by_id(id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Balance with ID {id} not found")))
        })
        .await
    }

    pub async fn list(
        &self,
        filters: BalanceFilterDto,
    ) -> Result<crate::dto::PaginationResult<PlayerPlanBalanceResponse>, AppError> {
        let mut result = self.repo.list(&filters).await?;
        self.expire_stale_in_results(&mut result.data).await?;
        Ok(result)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<PlayerPlanBalanceResponse, AppError> {
        self.repo
            .find_enriched_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Balance with ID {id} not found")))
    }

    pub async fn purchase_or_recharge(
        &self,
        dto: PurchaseBalanceDto,
        actor_id: Option<Uuid>,
    ) -> Result<PlayerPlanBalance, AppError> {
        let plan = self
            .plan_repo
            .find_by_id(dto.plan_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Plan with ID {} not found", dto.plan_id)))?;

        if !plan.is_active {
            return Err(AppError::BadRequest(
                "Cannot purchase an inactive plan".to_string(),
            ));
        }

        let kind = plan_kind_from_plan(&plan);
        let now = Utc::now();
        let fresh_expiry = now + Duration::days(plan.validity_days as i64);
        let deduction_snapshot = if plan.dynamic_deduction_enabled {
            plan.deduction_profile.as_ref()
        } else {
            None
        };

        let existing = self
            .repo
            .find_existing_for_scope(
                dto.player_id,
                plan.device_type.as_deref(),
                plan.device_sub_type.as_deref(),
                kind,
            )
            .await?;

        match existing {
            Some(balance) => {
                let new_expiry = if kind == plan_kind::HAPPY_HOURS
                    && balance.status == balance_status::ACTIVE
                    && balance.expiry_date > now
                {
                    balance.expiry_date
                } else {
                    fresh_expiry
                };

                let carry_forward = should_carry_forward_minutes(&balance, now);

                let updated = self
                    .repo
                    .recharge(
                        balance.id,
                        plan.time_credits,
                        new_expiry,
                        plan.id,
                        deduction_snapshot,
                        actor_id,
                        carry_forward,
                    )
                    .await?;

                self.ledger
                    .append(
                        updated.id,
                        dto.player_id,
                        plan.time_credits,
                        if carry_forward {
                            ledger_reason::RECHARGE
                        } else {
                            ledger_reason::PURCHASE
                        },
                        dto.transaction_id,
                        None,
                        updated.remaining_minutes,
                        updated.expiry_date,
                        actor_id,
                    )
                    .await?;

                Ok(updated)
            }
            None => {
                let balance = self
                    .repo
                    .create(
                        dto.player_id,
                        plan.device_type.as_deref(),
                        plan.device_sub_type.as_deref(),
                        kind,
                        plan.time_credits,
                        fresh_expiry,
                        plan.time_window_start,
                        plan.time_window_end,
                        plan.id,
                        plan.allowed_days.as_ref(),
                        plan.allowed_months.as_ref(),
                        deduction_snapshot,
                        actor_id,
                    )
                    .await?;

                self.ledger
                    .append(
                        balance.id,
                        dto.player_id,
                        plan.time_credits,
                        ledger_reason::PURCHASE,
                        dto.transaction_id,
                        None,
                        balance.remaining_minutes,
                        balance.expiry_date,
                        actor_id,
                    )
                    .await?;

                Ok(balance)
            }
        }
    }

    pub async fn validate_access(
        &self,
        balance_id: Uuid,
        device: Option<&Device>,
        current_time: Option<DateTime<Utc>>,
    ) -> Result<BalanceValidationResult, AppError> {
        let balance = self.get_raw(balance_id).await?;
        Ok(Self::validate_balance(&balance, device, current_time))
    }

    /// Whether balance purchase scope matches this kiosk device (exact type/subtype; NULL scope does not match).
    pub fn device_scope_matches(balance: &PlayerPlanBalance, device: &Device) -> bool {
        match (&balance.device_type, &balance.device_sub_type) {
            (Some(dt), Some(dst)) => dt == &device.device_type && dst == &device.device_sub_type,
            _ => false,
        }
    }

    /// Maps a failed validation to a contract `ErrorCode` string for `AppError::forbidden_code`.
    pub fn validation_failure_code(result: &BalanceValidationResult) -> &'static str {
        let reason = result.reason.as_deref().unwrap_or("");
        if reason.contains("expired") || reason.contains("Expired") {
            return "PLAN_EXPIRED";
        }
        if reason.contains("exhausted")
            || reason.contains("No minutes")
            || reason.contains("Insufficient")
        {
            return "PLAN_EXHAUSTED";
        }
        if reason.contains("time window")
            || reason.contains("allowed days")
            || reason.contains("allowed months")
            || reason.contains("Outside allowed")
        {
            return "TIME_WINDOW_VIOLATION";
        }
        if reason.contains("device") || reason.contains("Device") {
            return "DEVICE_TYPE_NOT_ALLOWED";
        }
        if reason.contains("Balance is") {
            return match reason {
                r if r.contains("cancelled") => "PLAN_CANCELLED",
                r if r.contains("expired") => "PLAN_EXPIRED",
                _ => "PLAN_NOT_ACTIVATED",
            };
        }
        "PLAN_NOT_ACTIVATED"
    }

    pub fn validation_to_app_error(result: BalanceValidationResult) -> AppError {
        AppError::forbidden_code(Self::validation_failure_code(&result))
    }

    pub async fn find_usable_for_device(
        &self,
        player_id: Uuid,
        device: &Device,
    ) -> Result<PlayerPlanBalance, AppError> {
        self.require_usable_for_device(player_id, device).await
    }

    pub async fn require_usable_for_device(
        &self,
        player_id: Uuid,
        device: &Device,
    ) -> Result<PlayerPlanBalance, AppError> {
        let result = self
            .list(BalanceFilterDto {
                player_id: Some(player_id),
                status: Some(balance_status::ACTIVE.to_string()),
                usable_only: Some(true),
                limit: Some(100),
                ..Default::default()
            })
            .await?;

        let mut best: Option<(PlayerPlanBalance, i32)> = None;
        let mut last_failure: Option<BalanceValidationResult> = None;
        let mut had_scope_match = false;

        for row in result.data {
            let balance = self.get_raw(row.id).await?;
            if !Self::device_scope_matches(&balance, device) {
                continue;
            }
            had_scope_match = true;
            let validation = Self::validate_balance(&balance, Some(device), None);
            if validation.valid {
                let minutes = balance.remaining_minutes;
                if best.as_ref().is_none_or(|(_, m)| minutes > *m) {
                    best = Some((balance, minutes));
                }
            } else {
                last_failure = Some(validation);
            }
        }

        if let Some((balance, _)) = best {
            return Ok(balance);
        }

        if !had_scope_match {
            return Err(AppError::forbidden_code("DEVICE_TYPE_NOT_ALLOWED"));
        }

        if let Some(failure) = last_failure {
            return Err(Self::validation_to_app_error(failure));
        }

        Err(AppError::forbidden_code("PLAN_NOT_ACTIVATED"))
    }

    pub async fn deduct_minutes(
        &self,
        balance_id: Uuid,
        minutes: i32,
        session_id: Option<Uuid>,
    ) -> Result<PlayerPlanBalance, AppError> {
        let updated = self.repo.deduct_minutes(balance_id, minutes).await?;

        self.invalidate_balance(&updated).await?;
        let cache_key = format!("balance:raw:{balance_id}");
        let _ = self.cache.delete(&[&cache_key]).await;

        self.ledger
            .append(
                updated.id,
                updated.player_id,
                -minutes,
                ledger_reason::SESSION_USAGE,
                None,
                session_id,
                updated.remaining_minutes,
                updated.expiry_date,
                None,
            )
            .await?;

        Ok(updated)
    }

    pub async fn get_best_balance(&self, player_id: Uuid) -> Result<PlayerPlanBalance, AppError> {
        let result = self
            .list(BalanceFilterDto {
                player_id: Some(player_id),
                status: Some(balance_status::ACTIVE.to_string()),
                ..Default::default()
            })
            .await?;

        if result.data.is_empty() {
            return Err(AppError::NotFound(format!(
                "No active balances found for player {player_id}"
            )));
        }

        let mut sorted = result.data;
        sorted.sort_by(|a, b| b.remaining_minutes.cmp(&a.remaining_minutes));

        let best = &sorted[0];
        self.get_raw(best.id).await
    }

    pub fn enforce_player_scope(
        filters: BalanceFilterDto,
        user_id: &str,
        is_admin: bool,
    ) -> Result<BalanceFilterDto, AppError> {
        if is_admin {
            return Ok(filters);
        }

        let user_uuid = Uuid::parse_str(user_id)
            .map_err(|_| AppError::Unauthorized("Authentication required".to_string()))?;

        if let Some(player_id) = filters.player_id {
            if player_id != user_uuid {
                return Err(AppError::Forbidden(
                    "You can only access your own balances".to_string(),
                ));
            }
        }

        Ok(BalanceFilterDto {
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
                "You can only access your own balances".to_string(),
            ));
        }

        Ok(())
    }

    pub fn validate_balance(
        balance: &PlayerPlanBalance,
        device: Option<&Device>,
        current_time: Option<DateTime<Utc>>,
    ) -> BalanceValidationResult {
        if let Some(dev) = device {
            if !Self::device_scope_matches(balance, dev) {
                return BalanceValidationResult {
                    valid: false,
                    reason: Some(format!(
                        "Plan not valid for device type {} / {}",
                        dev.device_type, dev.device_sub_type
                    )),
                };
            }
        }

        let now = current_time.unwrap_or_else(Utc::now);

        if balance.status != balance_status::ACTIVE {
            return BalanceValidationResult {
                valid: false,
                reason: Some(format!("Balance is {}", balance.status)),
            };
        }

        if now > balance.expiry_date {
            return BalanceValidationResult {
                valid: false,
                reason: Some("Balance expired".to_string()),
            };
        }

        if balance.remaining_minutes <= 0 {
            return BalanceValidationResult {
                valid: false,
                reason: Some("No minutes remaining".to_string()),
            };
        }

        if let Some(ref months_val) = balance.allowed_months {
            if let Some(months_arr) = months_val.as_array() {
                let current_month = now.month() as i64;
                let allowed: Vec<i64> = months_arr.iter().filter_map(|v| v.as_i64()).collect();
                if !allowed.is_empty() && !allowed.contains(&current_month) {
                    return BalanceValidationResult {
                        valid: false,
                        reason: Some(format!("Outside allowed months (current: {current_month})")),
                    };
                }
            }
        }

        if let Some(ref days_val) = balance.allowed_days {
            if let Some(days_arr) = days_val.as_array() {
                let weekday = now.weekday();
                let day_name = match weekday {
                    chrono::Weekday::Mon => "monday",
                    chrono::Weekday::Tue => "tuesday",
                    chrono::Weekday::Wed => "wednesday",
                    chrono::Weekday::Thu => "thursday",
                    chrono::Weekday::Fri => "friday",
                    chrono::Weekday::Sat => "saturday",
                    chrono::Weekday::Sun => "sunday",
                };
                let allowed: Vec<&str> = days_arr.iter().filter_map(|v| v.as_str()).collect();
                if !allowed.is_empty() && !allowed.contains(&day_name) {
                    return BalanceValidationResult {
                        valid: false,
                        reason: Some(format!("Outside allowed days (current: {day_name})")),
                    };
                }
            }
        }

        if let (Some(start), Some(end)) = (balance.window_start, balance.window_end) {
            let current = now.time();
            if current < start || current > end {
                return BalanceValidationResult {
                    valid: false,
                    reason: Some(format!(
                        "Outside allowed time window ({} - {})",
                        start.format("%H:%M:%S"),
                        end.format("%H:%M:%S")
                    )),
                };
            }
        }

        BalanceValidationResult {
            valid: true,
            reason: None,
        }
    }

    async fn expire_stale_in_results(
        &self,
        balances: &mut [PlayerPlanBalanceResponse],
    ) -> Result<(), AppError> {
        let now = Utc::now();
        for balance in balances.iter_mut() {
            if balance.status == balance_status::ACTIVE && balance.expiry_date < now {
                let _ = self
                    .repo
                    .set_status(balance.id, balance_status::EXPIRED)
                    .await;
                balance.status = balance_status::EXPIRED.to_string();
            }
        }
        Ok(())
    }
}

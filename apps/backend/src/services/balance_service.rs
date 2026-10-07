use chrono::{DateTime, Datelike, Utc};
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, keys, set_json, CacheService};
use crate::error::AppError;
use crate::models::{
    balance_status, plan_kind, BalanceFilterDto, BalanceValidationResult, Device,
    PlayerPlanBalance, PlayerPlanBalanceResponse, PurchaseBalanceDto,
};
use crate::repositories::TenantBalanceRepository;
use crate::tenancy::TenantDb;

pub struct BalanceService {
    cache: Arc<dyn CacheService>,
}

impl BalanceService {
    pub fn new(cache: Arc<dyn CacheService>) -> Self {
        Self { cache }
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

    async fn invalidate_balance_raw(&self, balance_id: Uuid) -> Result<(), AppError> {
        cache::invalidate(&*self.cache, &[keys::balance_raw(&balance_id)]).await
    }

    async fn write_through_balance_raw(&self, balance: &PlayerPlanBalance) -> Result<(), AppError> {
        let cache_key = keys::balance_raw(&balance.id);
        set_json(&*self.cache, &cache_key, balance, keys::ttl::SESSION).await
    }

    pub async fn sync_tenant_balance_cache_after_mutation(
        &self,
        db: Arc<TenantDb>,
        balance: &PlayerPlanBalance,
    ) -> Result<(), AppError> {
        self.invalidate_balance(balance).await?;
        self.invalidate_balance_raw(balance.id).await?;
        self.write_through_balance_raw(balance).await?;
        if let Some((session_id, device_id)) = TenantBalanceRepository::new(db)
            .find_open_session_ids(balance.player_id)
            .await?
        {
            cache::invalidate(
                &*self.cache,
                &[
                    keys::session_enriched(&session_id),
                    keys::session_device(&device_id),
                ],
            )
            .await?;
        }
        let _ = self
            .cache
            .invalidate_prefix(keys::SESSIONS_LIST_PREFIX)
            .await;
        Ok(())
    }

    pub async fn get_raw_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
    ) -> Result<PlayerPlanBalance, AppError> {
        TenantBalanceRepository::new(db)
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Balance with ID {id} not found")))
    }

    pub async fn list_tenant(
        &self,
        db: Arc<TenantDb>,
        filters: BalanceFilterDto,
    ) -> Result<crate::dto::PaginationResult<PlayerPlanBalanceResponse>, AppError> {
        let mut result = TenantBalanceRepository::new(db.clone())
            .list(&filters)
            .await?;
        let now = Utc::now();
        for balance in &mut result.data {
            if balance.status == balance_status::ACTIVE && balance.expiry_date < now {
                TenantBalanceRepository::new(db.clone())
                    .set_status(balance.id, balance_status::EXPIRED)
                    .await?;
                balance.status = balance_status::EXPIRED.into();
            }
        }
        Ok(result)
    }

    pub async fn get_by_id_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
    ) -> Result<PlayerPlanBalanceResponse, AppError> {
        Ok(self.get_raw_tenant(db, id).await?.into())
    }

    pub async fn purchase_or_recharge_tenant(
        &self,
        db: Arc<TenantDb>,
        dto: PurchaseBalanceDto,
        actor: Option<Uuid>,
    ) -> Result<PlayerPlanBalance, AppError> {
        let balance = TenantBalanceRepository::new(db.clone())
            .purchase_or_recharge(&dto, actor)
            .await?;
        if let Err(error) = self
            .sync_tenant_balance_cache_after_mutation(db, &balance)
            .await
        {
            tracing::warn!(%error, balance_id = %balance.id, "Committed tenant recharge cache refresh failed");
        }
        let _ = cache::invalidate_stats(&*self.cache).await;
        Ok(balance)
    }

    pub async fn get_best_balance_tenant(
        &self,
        db: Arc<TenantDb>,
        player_id: Uuid,
    ) -> Result<PlayerPlanBalance, AppError> {
        let result = self
            .list_tenant(
                db.clone(),
                BalanceFilterDto {
                    player_id: Some(player_id),
                    status: Some(balance_status::ACTIVE.into()),
                    usable_only: Some(true),
                    limit: Some(100),
                    ..Default::default()
                },
            )
            .await?;
        let id = result
            .data
            .into_iter()
            .max_by_key(|balance| balance.remaining_minutes)
            .map(|balance| balance.id)
            .ok_or_else(|| {
                AppError::NotFound(format!("No active balances found for player {player_id}"))
            })?;
        self.get_raw_tenant(db, id).await
    }

    pub async fn validate_access_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
        device: Option<&Device>,
        at: Option<DateTime<Utc>>,
    ) -> Result<BalanceValidationResult, AppError> {
        let balance = self.get_raw_tenant(db, id).await?;
        Ok(Self::validate_balance(&balance, device, at))
    }

    /// Whether balance purchase scope matches this kiosk device (exact type/subtype; NULL scope does not match except staff_allowance).
    pub fn device_scope_matches(balance: &PlayerPlanBalance, device: &Device) -> bool {
        if balance.kind == plan_kind::STAFF_ALLOWANCE {
            return true;
        }
        match (&balance.device_type, &balance.device_sub_type) {
            (Some(dt), Some(dst)) => dt == &device.device_type && dst == &device.device_sub_type,
            _ => false,
        }
    }

    /// Maps a failed validation to a contract `ErrorCode` string for `AppError::forbidden_code`.
    pub fn validation_failure_code(result: &BalanceValidationResult) -> &'static str {
        Self::validation_failure_code_for_kind(None, result)
    }

    pub fn validation_failure_code_for_balance(
        balance: &PlayerPlanBalance,
        result: &BalanceValidationResult,
    ) -> &'static str {
        Self::validation_failure_code_for_kind(Some(balance.kind.as_str()), result)
    }

    fn validation_failure_code_for_kind(
        kind: Option<&str>,
        result: &BalanceValidationResult,
    ) -> &'static str {
        if kind == Some(plan_kind::STAFF_ALLOWANCE) {
            let reason = result.reason.as_deref().unwrap_or("");
            if reason.contains("expired") || reason.contains("Expired") {
                return "STAFF_ALLOWANCE_EXPIRED";
            }
            if reason.contains("exhausted")
                || reason.contains("No minutes")
                || reason.contains("Insufficient")
            {
                return "STAFF_ALLOWANCE_EXHAUSTED";
            }
            return "STAFF_ALLOWANCE_NONE";
        }

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

    pub fn validation_to_app_error_for_balance(
        balance: &PlayerPlanBalance,
        result: BalanceValidationResult,
    ) -> AppError {
        AppError::forbidden_code(Self::validation_failure_code_for_balance(balance, &result))
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
}

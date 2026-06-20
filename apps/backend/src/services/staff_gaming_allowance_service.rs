use chrono::{Duration, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, keys, CacheService};
use crate::error::AppError;
use crate::models::{
    balance_status, ledger_reason, plan_kind, SetStaffGamingAllowanceDto,
    StaffGamingAllowanceStatus, StaffGamingAllowanceSummary, STAFF_ALLOWANCE_PERIOD_DAYS,
};
use crate::repositories::{BalanceRepository, LedgerRepository};
use crate::services::UserService;

pub struct StaffGamingAllowanceService {
    balances: BalanceRepository,
    ledger: LedgerRepository,
    users: Arc<UserService>,
    cache: Arc<dyn CacheService>,
}

impl StaffGamingAllowanceService {
    pub fn new(pool: PgPool, users: Arc<UserService>, cache: Arc<dyn CacheService>) -> Self {
        Self {
            balances: BalanceRepository::new(pool.clone()),
            ledger: LedgerRepository::new(pool),
            users,
            cache,
        }
    }

    async fn invalidate(&self, user_id: Uuid) -> Result<(), AppError> {
        cache::invalidate(
            &*self.cache,
            &[keys::balance_active(
                &user_id,
                &format!("null:null:{}", plan_kind::STAFF_ALLOWANCE),
            )],
        )
        .await
    }

    async fn ensure_staff_user(&self, user_id: Uuid) -> Result<(), AppError> {
        let user = self.users.get_by_id(user_id).await?;
        if user.role.as_deref() != Some("staff") {
            return Err(AppError::BadRequest(
                "Gaming allowance can only be configured for staff users".to_string(),
            ));
        }
        if !user.is_active {
            return Err(AppError::BadRequest(
                "Cannot configure allowance for inactive staff".to_string(),
            ));
        }
        Ok(())
    }

    fn summary_from_balance(
        user_id: Uuid,
        balance: &crate::models::PlayerPlanBalance,
        allotted_minutes: i32,
        now: chrono::DateTime<Utc>,
    ) -> StaffGamingAllowanceSummary {
        let used_minutes = (allotted_minutes - balance.remaining_minutes).max(0);
        let status = if balance.status == balance_status::ACTIVE && balance.expiry_date > now {
            if balance.remaining_minutes <= 0 {
                StaffGamingAllowanceStatus::Exhausted
            } else {
                StaffGamingAllowanceStatus::Active
            }
        } else if balance.status == balance_status::EXHAUSTED {
            StaffGamingAllowanceStatus::Exhausted
        } else {
            StaffGamingAllowanceStatus::Expired
        };

        StaffGamingAllowanceSummary {
            user_id,
            status,
            allotted_minutes,
            remaining_minutes: balance.remaining_minutes.max(0),
            used_minutes,
            period_start: Some(balance.created_at),
            period_end: Some(balance.expiry_date),
            balance_id: Some(balance.id),
        }
    }

    pub async fn get_summary(&self, user_id: Uuid) -> Result<StaffGamingAllowanceSummary, AppError> {
        self.ensure_staff_user(user_id).await?;
        let now = Utc::now();

        let Some(balance) = self.balances.find_latest_staff_allowance(user_id).await? else {
            return Ok(StaffGamingAllowanceSummary::none(user_id));
        };

        let allotted = self
            .ledger
            .find_grant_delta_for_balance(balance.id)
            .await?
            .unwrap_or(balance.remaining_minutes);

        Ok(Self::summary_from_balance(user_id, &balance, allotted, now))
    }

    pub async fn grant(
        &self,
        user_id: Uuid,
        dto: SetStaffGamingAllowanceDto,
        actor_id: Option<Uuid>,
    ) -> Result<StaffGamingAllowanceSummary, AppError> {
        self.ensure_staff_user(user_id).await?;

        if !dto.allotted_hours.is_finite() || dto.allotted_hours <= 0.0 {
            return Err(AppError::BadRequest(
                "allottedHours must be a positive number".to_string(),
            ));
        }

        let allotted_minutes = (dto.allotted_hours * 60.0).round() as i32;
        if allotted_minutes <= 0 {
            return Err(AppError::BadRequest(
                "allottedHours must convert to at least one minute".to_string(),
            ));
        }

        let had_prior = self
            .balances
            .find_latest_staff_allowance(user_id)
            .await?
            .is_some();

        if let Some(existing) = self.balances.find_active_staff_allowance(user_id).await? {
            self.balances
                .set_status(existing.id, balance_status::CANCELLED)
                .await?;
            self.ledger
                .append(
                    existing.id,
                    user_id,
                    -existing.remaining_minutes,
                    ledger_reason::ADJUSTMENT,
                    None,
                    None,
                    0,
                    existing.expiry_date,
                    actor_id,
                )
                .await?;
        }

        let now = Utc::now();
        let expiry = now + Duration::days(STAFF_ALLOWANCE_PERIOD_DAYS);
        let balance = self
            .balances
            .create_staff_allowance(user_id, allotted_minutes, expiry, actor_id)
            .await?;

        let reason = if had_prior {
            ledger_reason::STAFF_ALLOWANCE_RENEWAL
        } else {
            ledger_reason::STAFF_ALLOWANCE_GRANT
        };

        self.ledger
            .append(
                balance.id,
                user_id,
                allotted_minutes,
                reason,
                None,
                None,
                balance.remaining_minutes,
                balance.expiry_date,
                actor_id,
            )
            .await?;

        self.invalidate(user_id).await?;

        Ok(Self::summary_from_balance(
            user_id,
            &balance,
            allotted_minutes,
            now,
        ))
    }
}

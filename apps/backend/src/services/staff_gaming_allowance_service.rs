use crate::error::AppError;
use crate::models::{
    balance_status, SetStaffGamingAllowanceDto, StaffGamingAllowanceStatus,
    StaffGamingAllowanceSummary,
};
use crate::services::{BalanceService, ConfigService};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

pub struct StaffGamingAllowanceService {
    balance_service: Arc<BalanceService>,
    settings: Arc<ConfigService>,
}
impl StaffGamingAllowanceService {
    pub fn new(balance_service: Arc<BalanceService>, settings: Arc<ConfigService>) -> Self {
        Self {
            balance_service,
            settings,
        }
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

    pub async fn get_summary_tenant(
        &self,
        db: Arc<crate::tenancy::TenantDb>,
        user: Uuid,
    ) -> Result<StaffGamingAllowanceSummary, AppError> {
        Ok(
            match crate::repositories::TenantBalanceRepository::new(db)
                .staff_allowance_summary(user)
                .await?
            {
                Some((balance, minutes)) => {
                    Self::summary_from_balance(user, &balance, minutes, Utc::now())
                }
                None => StaffGamingAllowanceSummary::none(user),
            },
        )
    }
    pub async fn grant_tenant(
        &self,
        db: Arc<crate::tenancy::TenantDb>,
        user: Uuid,
        dto: SetStaffGamingAllowanceDto,
        actor: Option<Uuid>,
    ) -> Result<StaffGamingAllowanceSummary, AppError> {
        let minutes = (dto.allotted_hours * 60.0).round();
        if !dto.allotted_hours.is_finite()
            || dto.allotted_hours <= 0.0
            || !(1.0..=i32::MAX as f64).contains(&minutes)
        {
            return Err(AppError::BadRequest(
                "allottedHours must convert to between 1 and 2147483647 minutes".into(),
            ));
        }
        let values = self
            .settings
            .effective_tenant(
                db.clone(),
                db.tenant_id(),
                crate::models::EffectiveSettingsQuery {
                    location_id: None,
                    category: Some("staff".into()),
                },
            )
            .await?;
        let days = values
            .into_iter()
            .find(|value| value.key == "staff.allowance_period_days")
            .and_then(|value| value.value.as_i64())
            .ok_or_else(|| AppError::Internal("Staff allowance period is unavailable".into()))?;
        let balance = crate::repositories::TenantBalanceRepository::new(db.clone())
            .grant_staff_allowance(user, minutes as i32, days, actor)
            .await?;
        if let Err(error) = self
            .balance_service
            .sync_tenant_balance_cache_after_mutation(db, &balance)
            .await
        {
            tracing::warn!(%error, %user, "Committed staff allowance cache refresh failed");
        }
        Ok(Self::summary_from_balance(
            user,
            &balance,
            minutes as i32,
            Utc::now(),
        ))
    }
}

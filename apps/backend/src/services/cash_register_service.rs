use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, get_or_set, keys, CacheService};
use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CashRegister, CashRegisterEntry, CashRegisterFilterDto, CashRegisterWithEntries,
    CloseCashRegisterDto, CreateCashRegisterEntryDto, OpenCashRegisterDto,
};
use crate::repositories::CashRegisterRepository;
use crate::services::{NotificationService, RecordNotification, Recipients};
use crate::models::activity_kind;

pub struct CashRegisterService {
    repo: CashRegisterRepository,
    pool: PgPool,
    cache: Arc<dyn CacheService>,
    notifications: Option<NotificationService>,
}

impl CashRegisterService {
    pub fn new(pool: PgPool, cache: Arc<dyn CacheService>) -> Self {
        Self {
            repo: CashRegisterRepository::new(pool.clone()),
            pool,
            cache,
            notifications: None,
        }
    }

    pub fn with_notifications(mut self, notifications: NotificationService) -> Self {
        self.notifications = Some(notifications);
        self
    }

    async fn invalidate_register(&self, register_id: Uuid) -> Result<(), AppError> {
        cache::invalidate(
            &*self.cache,
            &[keys::cash_register_totals(&register_id)],
        )
        .await
    }

    pub async fn open(
        &self,
        dto: OpenCashRegisterDto,
        actor_id: Uuid,
    ) -> Result<CashRegister, AppError> {
        self.validate_shift_active(dto.shift_id).await?;

        let existing = self.repo.find_by_shift(dto.shift_id).await?;
        if let Some(ref reg) = existing {
            if reg.status == "open" {
                return Err(AppError::Conflict(
                    "An open cash register already exists for this shift".to_string(),
                ));
            }
        }

        let result = self.repo.open_register(&dto, actor_id).await?;
        self.invalidate_register(result.id).await?;
        if let Some(ref notifications) = self.notifications {
            let _ = notifications
                .record(RecordNotification {
                    kind: activity_kind::CASH_REGISTER_OPENED.to_string(),
                    title: "Cash register opened".to_string(),
                    summary: None,
                    payload: serde_json::json!({
                        "registerId": result.id.to_string(),
                        "shiftId": dto.shift_id.to_string(),
                    }),
                    actor_user_id: Some(actor_id),
                    entity_type: Some("cash_register".to_string()),
                    entity_id: Some(result.id),
                    recipients: Recipients::AdminAndUsers(vec![actor_id]),
                })
                .await;
        }
        Ok(result)
    }

    pub async fn close(
        &self,
        id: Uuid,
        dto: CloseCashRegisterDto,
        actor_id: Uuid,
    ) -> Result<CashRegister, AppError> {
        let register =
            self.repo.find_by_id(id).await?.ok_or_else(|| {
                AppError::NotFound(format!("Cash register with ID {id} not found"))
            })?;

        if register.status != "open" {
            return Err(AppError::BadRequest(
                "Cash register is not open".to_string(),
            ));
        }

        let result = self.repo.close_register(id, &dto, actor_id).await?;
        self.invalidate_register(id).await?;
        if let Some(ref notifications) = self.notifications {
            let _ = notifications
                .record(RecordNotification {
                    kind: activity_kind::CASH_REGISTER_CLOSED.to_string(),
                    title: "Cash register closed".to_string(),
                    summary: dto.notes.clone(),
                    payload: serde_json::json!({
                        "registerId": result.id.to_string(),
                        "shiftId": result.shift_id.to_string(),
                    }),
                    actor_user_id: Some(actor_id),
                    entity_type: Some("cash_register".to_string()),
                    entity_id: Some(result.id),
                    recipients: Recipients::AdminAndUsers(vec![actor_id]),
                })
                .await;
        }
        Ok(result)
    }

    pub async fn reconcile(
        &self,
        id: Uuid,
        dto: crate::models::ReconcileCashRegisterDto,
        actor_id: Uuid,
    ) -> Result<CashRegister, AppError> {
        let register =
            self.repo.find_by_id(id).await?.ok_or_else(|| {
                AppError::NotFound(format!("Cash register with ID {id} not found"))
            })?;

        if register.status != "closed" {
            return Err(AppError::BadRequest(
                "Only closed cash registers can be reconciled".to_string(),
            ));
        }

        let result = self
            .repo
            .reconcile(id, dto.reconciliation_notes, actor_id)
            .await?;
        self.invalidate_register(id).await?;
        Ok(result)
    }

    pub async fn update_opening_balance(
        &self,
        id: Uuid,
        dto: crate::models::UpdateOpeningBalanceDto,
        actor_id: Uuid,
    ) -> Result<CashRegister, AppError> {
        let register =
            self.repo.find_by_id(id).await?.ok_or_else(|| {
                AppError::NotFound(format!("Cash register with ID {id} not found"))
            })?;

        if register.status != "open" {
            return Err(AppError::BadRequest(
                "Only open cash registers can have their opening balance updated".to_string(),
            ));
        }

        let result = self
            .repo
            .update_opening_balance(id, dto.opening_balance, dto.opening_denominations, actor_id)
            .await?;
        self.invalidate_register(id).await?;
        Ok(result)
    }

    pub async fn add_entry(
        &self,
        register_id: Uuid,
        dto: CreateCashRegisterEntryDto,
        actor_id: Uuid,
    ) -> Result<CashRegisterEntry, AppError> {
        let register = self.repo.find_by_id(register_id).await?.ok_or_else(|| {
            AppError::NotFound(format!("Cash register with ID {register_id} not found"))
        })?;

        if register.status != "open" {
            return Err(AppError::BadRequest(
                "Cannot add entries to a closed cash register".to_string(),
            ));
        }

        let entry = self.repo.add_entry(register_id, &dto, actor_id).await?;
        self.invalidate_register(register_id).await?;
        Ok(entry)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<CashRegisterWithEntries, AppError> {
        let register =
            self.repo.find_by_id(id).await?.ok_or_else(|| {
                AppError::NotFound(format!("Cash register with ID {id} not found"))
            })?;

        let entries = self.repo.list_entries(id).await?;

        Ok(CashRegisterWithEntries { register, entries })
    }

    pub async fn get_by_shift(&self, shift_id: Uuid) -> Result<CashRegisterWithEntries, AppError> {
        let register = self.repo.find_by_shift(shift_id).await?.ok_or_else(|| {
            AppError::NotFound(format!("Cash register for shift {shift_id} not found"))
        })?;

        let entries = self.repo.list_entries(register.id).await?;

        Ok(CashRegisterWithEntries { register, entries })
    }

    pub async fn list(
        &self,
        filters: CashRegisterFilterDto,
    ) -> Result<PaginationResult<CashRegister>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn get_expected_closing(&self, register_id: Uuid) -> Result<f64, AppError> {
        let cache_key = keys::cash_register_totals(&register_id);
        get_or_set(&*self.cache, &cache_key, keys::ttl::AGGREGATE, || async {
            self.repo.get_expected_closing(register_id).await
        })
        .await
    }

    pub async fn find_register_by_shift(
        &self,
        shift_id: Uuid,
    ) -> Result<Option<CashRegister>, AppError> {
        self.repo.find_by_shift(shift_id).await
    }

    /// Opening float after a closed shift: pre-deposit closing count minus all deposit withdrawals.
    pub fn opening_from_closing_and_deposits(closing: f64, deposit_cash_out_total: f64) -> f64 {
        closing - deposit_cash_out_total
    }

    pub async fn compute_carry_forward_opening(
        &self,
        register: &CashRegister,
    ) -> Result<f64, AppError> {
        let closing = register.closing_balance.unwrap_or(0.0);
        let withdrawn = self.repo.sum_all_deposit_cash_out(register.id).await?;
        Ok(Self::opening_from_closing_and_deposits(closing, withdrawn))
    }

    pub async fn carry_forward_balance(
        &self,
        user_id: Uuid,
        new_shift_id: Uuid,
        actor_id: Uuid,
    ) -> Result<CashRegister, AppError> {
        let last_register = self.repo.find_last_closed_by_user(user_id).await?;
        let opening = match last_register {
            Some(ref r) => self.compute_carry_forward_opening(r).await?,
            None => 0.0,
        };
        self.ensure_open_for_shift(new_shift_id, actor_id, opening)
            .await
    }

    pub async fn ensure_open_for_shift(
        &self,
        shift_id: Uuid,
        actor_id: Uuid,
        opening_balance: f64,
    ) -> Result<CashRegister, AppError> {
        if let Some(existing) = self.repo.find_by_shift(shift_id).await? {
            if existing.status == "open" {
                return Ok(existing);
            }
        }

        self.open(
            OpenCashRegisterDto {
                shift_id,
                opening_balance,
                opening_denominations: None,
                notes: Some("Auto-opened for shift".to_string()),
            },
            actor_id,
        )
        .await
    }

    async fn validate_shift_active(&self, shift_id: Uuid) -> Result<(), AppError> {
        let row: Option<(String,)> = sqlx::query_as(r#"SELECT status FROM shifts WHERE id = $1"#)
            .bind(shift_id)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            None => Err(AppError::NotFound(format!(
                "Shift with ID {shift_id} not found"
            ))),
            Some((status,)) if status != "active" => Err(AppError::BadRequest(format!(
                "Shift is not active (status: {status})"
            ))),
            _ => Ok(()),
        }
    }
}

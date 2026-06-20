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

pub struct CashRegisterService {
    repo: CashRegisterRepository,
    pool: PgPool,
    cache: Arc<dyn CacheService>,
}

impl CashRegisterService {
    pub fn new(pool: PgPool, cache: Arc<dyn CacheService>) -> Self {
        Self {
            repo: CashRegisterRepository::new(pool.clone()),
            pool,
            cache,
        }
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

        self.repo.open_register(&dto, actor_id).await
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

        self.repo.close_register(id, &dto, actor_id).await
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

        self.repo
            .reconcile(id, dto.reconciliation_notes, actor_id)
            .await
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

        self.repo
            .update_opening_balance(id, dto.opening_balance, dto.opening_denominations, actor_id)
            .await
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

    pub async fn carry_forward_balance(
        &self,
        user_id: Uuid,
        new_shift_id: Uuid,
        actor_id: Uuid,
    ) -> Result<CashRegister, AppError> {
        let last_register = self.repo.find_last_closed_by_user(user_id).await?;
        let opening = match last_register {
            Some(ref r) => {
                let closing = r.closing_balance.unwrap_or(0.0);
                let deposited = self.repo.sum_deposit_entries(r.id).await?;
                closing - deposited
            }
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

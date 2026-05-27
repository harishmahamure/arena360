use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CashDeposit, CashDepositFilterDto, CreateCashRegisterEntryDto, InitiateDepositDto,
};
use crate::repositories::{CashDepositRepository, CashRegisterRepository};

pub struct CashDepositService {
    repo: CashDepositRepository,
    cash_register_repo: CashRegisterRepository,
}

impl CashDepositService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: CashDepositRepository::new(pool.clone()),
            cash_register_repo: CashRegisterRepository::new(pool),
        }
    }

    pub async fn initiate(
        &self,
        dto: InitiateDepositDto,
        staff_id: Uuid,
    ) -> Result<CashDeposit, AppError> {
        if dto.amount <= 0.0 {
            return Err(AppError::BadRequest(
                "Deposit amount must be greater than zero".to_string(),
            ));
        }

        let register = self
            .cash_register_repo
            .find_by_id(dto.cash_register_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "Cash register with ID {} not found",
                    dto.cash_register_id
                ))
            })?;

        if register.status != "open" {
            return Err(AppError::BadRequest(
                "Cannot initiate deposit on a closed cash register".to_string(),
            ));
        }

        if register.shift_id != dto.shift_id {
            return Err(AppError::BadRequest(
                "Cash register does not belong to the specified shift".to_string(),
            ));
        }

        let deposit = self.repo.create(&dto, staff_id).await?;

        let entry = CreateCashRegisterEntryDto {
            entry_type: "cash_out".to_string(),
            amount: dto.amount,
            reason: Some(format!("Cash deposit pending approval ({})", deposit.id)),
            reference_id: Some(deposit.id),
            reference_type: Some("cash_deposit".to_string()),
        };

        self.cash_register_repo
            .add_entry(dto.cash_register_id, &entry, staff_id)
            .await?;

        Ok(deposit)
    }

    pub async fn approve(
        &self,
        id: Uuid,
        deposit_type: &str,
        admin_id: Uuid,
    ) -> Result<CashDeposit, AppError> {
        if deposit_type != "bank" && deposit_type != "home" {
            return Err(AppError::BadRequest(
                "Deposit type must be 'bank' or 'home'".to_string(),
            ));
        }

        self.repo.approve(id, deposit_type, admin_id).await
    }

    pub async fn reject(
        &self,
        id: Uuid,
        rejection_reason: &str,
        admin_id: Uuid,
    ) -> Result<CashDeposit, AppError> {
        if rejection_reason.trim().is_empty() {
            return Err(AppError::BadRequest(
                "Rejection reason is required".to_string(),
            ));
        }

        self.repo.reject(id, rejection_reason, admin_id).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<CashDeposit, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Deposit with ID {id} not found")))
    }

    pub async fn list(
        &self,
        filters: CashDepositFilterDto,
    ) -> Result<PaginationResult<CashDeposit>, AppError> {
        self.repo.list(&filters).await
    }
}

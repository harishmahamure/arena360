use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;
use serde_json::json;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CashDeposit, CashDepositFilterDto, CreateCashRegisterEntryDto, InitiateDepositDto,
};
use crate::realtime::OutboxService;
use crate::repositories::{CashDepositRepository, CashRegisterRepository};
use crate::services::CashRegisterService;
use crate::services::NotificationService;
use crate::models::activity_kind;
use crate::services::{RecordNotification, Recipients};

pub struct CashDepositService {
    repo: CashDepositRepository,
    cash_register_repo: CashRegisterRepository,
    cash_registers: Option<Arc<CashRegisterService>>,
    outbox: OutboxService,
    notifications: NotificationService,
}

impl CashDepositService {
    pub fn new(
        pool: PgPool,
        outbox: OutboxService,
        notifications: NotificationService,
        cash_registers: Arc<CashRegisterService>,
    ) -> Self {
        Self {
            repo: CashDepositRepository::new(pool.clone()),
            cash_register_repo: CashRegisterRepository::new(pool),
            cash_registers: Some(cash_registers),
            outbox,
            notifications,
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

        let payload = serde_json::json!({
            "deposit_id": deposit.id.to_string(),
            "amount": deposit.amount,
            "staff_id": staff_id.to_string(),
            "entity_type": "cash_deposit",
        });
        let _ = self
            .outbox
            .publish(
                "admin",
                "approval.requested",
                payload.clone(),
                Some("admin"),
                None,
                true,
            )
            .await;
        let _ = self
            .notifications
            .record_approval_requested(
                "cash_deposit",
                deposit.id,
                &format!("Cash deposit approval: ₹{:.2}", deposit.amount),
                payload,
                Some(staff_id),
            )
            .await;
        let _ = self
            .notifications
            .record(RecordNotification {
                kind: activity_kind::CASH_DEPOSIT_INITIATED.to_string(),
                title: format!("Cash deposit initiated: ₹{:.2}", deposit.amount),
                summary: Some("Pending admin approval".to_string()),
                payload: json!({
                    "depositId": deposit.id.to_string(),
                    "amount": deposit.amount,
                }),
                actor_user_id: Some(staff_id),
                entity_type: Some("cash_deposit".to_string()),
                entity_id: Some(deposit.id),
                recipients: Recipients::Users(vec![staff_id]),
            })
            .await;

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

        let deposit = self.repo.approve(id, deposit_type, admin_id).await?;

        let payload = serde_json::json!({
            "deposit_id": deposit.id.to_string(),
            "status": "approved",
            "entity_type": "cash_deposit",
        });
        let _ = self
            .outbox
            .publish(
                &format!("user:{}", deposit.initiated_by),
                "approval.decided",
                payload.clone(),
                None,
                Some(deposit.initiated_by),
                true,
            )
            .await;
        let _ = self
            .outbox
            .publish(
                "admin",
                "cash_deposit.status_changed",
                payload.clone(),
                Some("admin"),
                None,
                true,
            )
            .await;
        let _ = self
            .notifications
            .record_approval_decided(
                "cash_deposit",
                deposit.id,
                "approved",
                "Cash deposit approved",
                payload,
                deposit.initiated_by,
                Some(admin_id),
            )
            .await;

        self.recalculate_register_closure(deposit.cash_register_id)
            .await?;

        Ok(deposit)
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

        let deposit = self.repo.reject(id, rejection_reason, admin_id).await?;

        {
            let payload = serde_json::json!({
                "deposit_id": deposit.id.to_string(),
                "status": "rejected",
                "entity_type": "cash_deposit",
            });
            let _ = self
                .outbox
                .publish(
                    &format!("user:{}", deposit.initiated_by),
                    "approval.decided",
                    payload.clone(),
                    None,
                    Some(deposit.initiated_by),
                    true,
                )
                .await;
            let _ = self
                .outbox
                .publish(
                    "admin",
                    "cash_deposit.status_changed",
                    payload.clone(),
                    Some("admin"),
                    None,
                    true,
                )
                .await;
            let _ = self
                .notifications
                .record_approval_decided(
                    "cash_deposit",
                    deposit.id,
                    "rejected",
                    "Cash deposit rejected",
                    payload,
                    deposit.initiated_by,
                    Some(admin_id),
                )
                .await;
        }

        let register = self
            .cash_register_repo
            .find_by_id(deposit.cash_register_id)
            .await?;
        if let Some(reg) = register {
            if reg.status == "open" {
                let _ = self
                    .cash_register_repo
                    .add_entry(
                        deposit.cash_register_id,
                        &CreateCashRegisterEntryDto {
                            entry_type: "cash_in".to_string(),
                            amount: deposit.amount,
                            reason: Some(format!("Deposit {} rejected – reversal", deposit.id)),
                            reference_id: Some(deposit.id),
                            reference_type: Some("cash_deposit".to_string()),
                        },
                        admin_id,
                    )
                    .await;
            }
        }

        self.recalculate_register_closure(deposit.cash_register_id)
            .await?;

        Ok(deposit)
    }

    async fn recalculate_register_closure(&self, register_id: Uuid) -> Result<(), AppError> {
        if let Some(ref cash_registers) = self.cash_registers {
            cash_registers
                .recalculate_closure_after_deposit(register_id)
                .await?;
        }
        Ok(())
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

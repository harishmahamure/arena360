use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CreateCashRegisterEntryDto, CreateExpenseDto, Expense, ExpenseFilterDto, ExpenseSummaryDto,
    UpdateExpenseDto,
};
use crate::realtime::OutboxService;
use crate::repositories::{ExpenseCategoryRepository, ExpenseRepository, VendorRepository};
use crate::services::cash_register_service::CashRegisterService;
use crate::services::shift_service::ShiftService;
use crate::services::NotificationService;

pub struct ExpenseService {
    repo: ExpenseRepository,
    category_repo: ExpenseCategoryRepository,
    vendor_repo: VendorRepository,
    cash_register_service: CashRegisterService,
    shift_service: ShiftService,
    outbox: OutboxService,
    notifications: NotificationService,
}

impl ExpenseService {
    pub fn new(
        pool: PgPool,
        cash_register_service: CashRegisterService,
        shift_service: ShiftService,
        outbox: OutboxService,
        notifications: NotificationService,
    ) -> Self {
        Self {
            repo: ExpenseRepository::new(pool.clone()),
            category_repo: ExpenseCategoryRepository::new(pool.clone()),
            vendor_repo: VendorRepository::new(pool),
            cash_register_service,
            shift_service,
            outbox,
            notifications,
        }
    }

    pub async fn list(
        &self,
        filters: ExpenseFilterDto,
    ) -> Result<PaginationResult<Expense>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Expense, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Expense with ID {id} not found")))
    }

    pub async fn create(
        &self,
        dto: CreateExpenseDto,
        created_by: Option<Uuid>,
    ) -> Result<Expense, AppError> {
        if dto.amount <= 0.0 {
            return Err(AppError::BadRequest(
                "Amount must be greater than 0".to_string(),
            ));
        }

        self.category_repo
            .find_by_id(dto.category_id)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!(
                    "Expense category with ID {} not found",
                    dto.category_id
                ))
            })?;

        if let Some(vendor_id) = dto.vendor_id {
            self.vendor_repo
                .find_by_id(vendor_id)
                .await?
                .ok_or_else(|| {
                    AppError::BadRequest(format!("Vendor with ID {vendor_id} not found"))
                })?;
        }

        let expense = self.repo.create(&dto, created_by).await?;

        if expense.approval_status == "pending" {
            let payload = serde_json::json!({
                "expense_id": expense.id.to_string(),
                "amount": expense.amount,
                "entity_type": "expense",
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
                    "expense",
                    expense.id,
                    &format!("Expense approval: ₹{:.2}", expense.amount),
                    payload,
                    created_by,
                )
                .await;
        }

        Ok(expense)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: UpdateExpenseDto,
        updated_by: Option<Uuid>,
    ) -> Result<Expense, AppError> {
        let existing = self
            .repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Expense with ID {id} not found")))?;

        if existing.approval_status == "approved" {
            return Err(AppError::BadRequest(
                "Cannot update an approved expense".to_string(),
            ));
        }

        if let Some(amount) = dto.amount {
            if amount <= 0.0 {
                return Err(AppError::BadRequest(
                    "Amount must be greater than 0".to_string(),
                ));
            }
        }

        if let Some(category_id) = dto.category_id {
            self.category_repo
                .find_by_id(category_id)
                .await?
                .ok_or_else(|| {
                    AppError::BadRequest(format!(
                        "Expense category with ID {category_id} not found"
                    ))
                })?;
        }

        if let Some(vendor_id) = dto.vendor_id {
            self.vendor_repo
                .find_by_id(vendor_id)
                .await?
                .ok_or_else(|| {
                    AppError::BadRequest(format!("Vendor with ID {vendor_id} not found"))
                })?;
        }

        self.repo.update(id, &dto, updated_by).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<Expense, AppError> {
        self.repo.soft_delete(id).await
    }

    pub async fn approve(&self, id: Uuid, approved_by: Uuid) -> Result<Expense, AppError> {
        let existing = self
            .repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Expense with ID {id} not found")))?;

        if existing.approval_status == "approved" {
            return Err(AppError::BadRequest(
                "Expense is already approved".to_string(),
            ));
        }

        let expense = self.repo.approve(id, approved_by).await?;

        if expense.payment_method == "cash" || expense.payment_method == "split_payment" {
            if let Ok(Some(active_shift)) = self.shift_service.get_active(approved_by).await {
                if let Ok(register_with_entries) = self
                    .cash_register_service
                    .get_by_shift(active_shift.id)
                    .await
                {
                    let entry_dto = CreateCashRegisterEntryDto {
                        entry_type: "cash_out".to_string(),
                        amount: expense.amount,
                        reason: Some(format!(
                            "Expense: {}",
                            expense.description.as_deref().unwrap_or("N/A")
                        )),
                        reference_id: Some(expense.id),
                        reference_type: Some("expense".to_string()),
                    };

                    if let Ok(entry) = self
                        .cash_register_service
                        .add_entry(register_with_entries.register.id, entry_dto, approved_by)
                        .await
                    {
                        let _ = self
                            .repo
                            .set_cash_register_entry_id(expense.id, entry.id)
                            .await;
                    }
                }
            }
        }

        {
            let payload = serde_json::json!({
                "expense_id": expense.id.to_string(),
                "status": "approved",
                "entity_type": "expense",
            });
            if let Some(created_by) = expense.created_by {
                let _ = self
                    .outbox
                    .publish(
                        &format!("user:{created_by}"),
                        "approval.decided",
                        payload.clone(),
                        None,
                        Some(created_by),
                        true,
                    )
                    .await;
            }
            let _ = self
                .outbox
                .publish(
                    "admin",
                    "expense.status_changed",
                    payload.clone(),
                    Some("admin"),
                    None,
                    true,
                )
                .await;
            if let Some(created_by) = expense.created_by {
                let _ = self
                    .notifications
                    .record_approval_decided(
                        "expense",
                        expense.id,
                        "approved",
                        "Expense approved",
                        payload,
                        created_by,
                        Some(approved_by),
                    )
                    .await;
            }
        }

        Ok(expense)
    }

    pub async fn reject(
        &self,
        id: Uuid,
        rejection_reason: &str,
        rejected_by: Uuid,
    ) -> Result<Expense, AppError> {
        let existing = self
            .repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Expense with ID {id} not found")))?;

        if existing.approval_status == "approved" {
            return Err(AppError::BadRequest(
                "Cannot reject an already approved expense".to_string(),
            ));
        }

        let expense = self.repo.reject(id, rejection_reason, rejected_by).await?;

        {
            let payload = serde_json::json!({
                "expense_id": expense.id.to_string(),
                "status": "rejected",
                "entity_type": "expense",
            });
            if let Some(created_by) = expense.created_by {
                let _ = self
                    .outbox
                    .publish(
                        &format!("user:{created_by}"),
                        "approval.decided",
                        payload.clone(),
                        None,
                        Some(created_by),
                        true,
                    )
                    .await;
            }
            let _ = self
                .outbox
                .publish(
                    "admin",
                    "expense.status_changed",
                    payload.clone(),
                    Some("admin"),
                    None,
                    true,
                )
                .await;
            if let Some(created_by) = expense.created_by {
                let _ = self
                    .notifications
                    .record_approval_decided(
                        "expense",
                        expense.id,
                        "rejected",
                        "Expense rejected",
                        payload,
                        created_by,
                        Some(rejected_by),
                    )
                    .await;
            }
        }

        Ok(expense)
    }

    pub async fn get_summary(&self) -> Result<Vec<ExpenseSummaryDto>, AppError> {
        self.repo.get_summary_by_category().await
    }
}

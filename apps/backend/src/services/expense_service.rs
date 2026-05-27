use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CreateExpenseDto, Expense, ExpenseFilterDto, ExpenseSummaryDto, UpdateExpenseDto,
};
use crate::repositories::{ExpenseCategoryRepository, ExpenseRepository, VendorRepository};

pub struct ExpenseService {
    repo: ExpenseRepository,
    category_repo: ExpenseCategoryRepository,
    vendor_repo: VendorRepository,
}

impl ExpenseService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: ExpenseRepository::new(pool.clone()),
            category_repo: ExpenseCategoryRepository::new(pool.clone()),
            vendor_repo: VendorRepository::new(pool),
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

        self.repo.create(&dto, created_by).await
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

        self.repo.approve(id, approved_by).await
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

        self.repo.reject(id, rejection_reason, rejected_by).await
    }

    pub async fn get_summary(&self) -> Result<Vec<ExpenseSummaryDto>, AppError> {
        self.repo.get_summary_by_category().await
    }
}

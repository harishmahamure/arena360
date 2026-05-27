use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CreateExpenseCategoryDto, ExpenseCategory, ExpenseCategoryFilterDto, UpdateExpenseCategoryDto,
};
use crate::repositories::ExpenseCategoryRepository;

pub struct ExpenseCategoryService {
    repo: ExpenseCategoryRepository,
}

impl ExpenseCategoryService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: ExpenseCategoryRepository::new(pool),
        }
    }

    pub async fn list(
        &self,
        filters: ExpenseCategoryFilterDto,
    ) -> Result<PaginationResult<ExpenseCategory>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<ExpenseCategory, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Expense category with ID {id} not found")))
    }

    pub async fn create(
        &self,
        dto: CreateExpenseCategoryDto,
        created_by: Option<Uuid>,
    ) -> Result<ExpenseCategory, AppError> {
        if self.repo.name_exists(&dto.name, None).await? {
            return Err(AppError::Conflict(format!(
                "Expense category '{}' already exists",
                dto.name
            )));
        }

        if let Some(parent_id) = dto.parent_id {
            self.repo.find_by_id(parent_id).await?.ok_or_else(|| {
                AppError::BadRequest(format!("Parent category with ID {parent_id} not found"))
            })?;
        }

        self.repo.create(&dto, created_by).await
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: UpdateExpenseCategoryDto,
        updated_by: Option<Uuid>,
    ) -> Result<ExpenseCategory, AppError> {
        if let Some(name) = &dto.name {
            if self.repo.name_exists(name, Some(id)).await? {
                return Err(AppError::Conflict(format!(
                    "Expense category '{name}' already exists"
                )));
            }
        }

        if let Some(parent_id) = dto.parent_id {
            if parent_id == id {
                return Err(AppError::BadRequest(
                    "Category cannot be its own parent".to_string(),
                ));
            }
            self.repo.find_by_id(parent_id).await?.ok_or_else(|| {
                AppError::BadRequest(format!("Parent category with ID {parent_id} not found"))
            })?;
        }

        self.repo.update(id, &dto, updated_by).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        self.repo.delete(id).await
    }
}

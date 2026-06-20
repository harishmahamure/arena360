use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, get_or_set, keys, CacheService};
use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CreateExpenseCategoryDto, ExpenseCategory, ExpenseCategoryFilterDto, UpdateExpenseCategoryDto,
};
use crate::repositories::ExpenseCategoryRepository;

pub struct ExpenseCategoryService {
    repo: ExpenseCategoryRepository,
    cache: Arc<dyn CacheService>,
}

impl ExpenseCategoryService {
    pub fn new(pool: PgPool, cache: Arc<dyn CacheService>) -> Self {
        Self {
            repo: ExpenseCategoryRepository::new(pool),
            cache,
        }
    }

    async fn invalidate_categories(&self, id: Option<Uuid>) -> Result<(), AppError> {
        let mut cache_keys = vec![keys::expense_categories_tree().to_string()];
        if let Some(id) = id {
            cache_keys.push(keys::expense_category(&id));
        }
        cache::invalidate(&*self.cache, &cache_keys).await?;
        self.cache.invalidate_prefix("expense_categories:list:").await
    }

    pub async fn list(
        &self,
        filters: ExpenseCategoryFilterDto,
    ) -> Result<PaginationResult<ExpenseCategory>, AppError> {
        let cache_key = format!(
            "expense_categories:list:{}",
            keys::filter_hash(&filters)
        );
        get_or_set(&*self.cache, &cache_key, keys::ttl::LOOKUP, || async {
            self.repo.list(&filters).await
        })
        .await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<ExpenseCategory, AppError> {
        let cache_key = keys::expense_category(&id);
        get_or_set(&*self.cache, &cache_key, keys::ttl::LOOKUP, || async {
            self.repo
                .find_by_id(id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Expense category with ID {id} not found")))
        })
        .await
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

        let category = self.repo.create(&dto, created_by).await?;
        self.invalidate_categories(Some(category.id)).await?;
        Ok(category)
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

        let category = self.repo.update(id, &dto, updated_by).await?;
        self.invalidate_categories(Some(id)).await?;
        Ok(category)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        self.repo.delete(id).await?;
        self.invalidate_categories(Some(id)).await
    }
}

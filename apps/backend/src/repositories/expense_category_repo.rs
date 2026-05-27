use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CreateExpenseCategoryDto, ExpenseCategory, ExpenseCategoryFilterDto, UpdateExpenseCategoryDto,
};

pub struct ExpenseCategoryRepository {
    pool: PgPool,
}

impl ExpenseCategoryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id, name, description,
               "parentId" as parent_id,
               "isActive" as is_active,
               "budgetAmount"::float8 as budget_amount,
               "budgetPeriod" as budget_period,
               "createdBy" as created_by,
               "updatedBy" as updated_by,
               "createdAt" as created_at,
               "updatedAt" as updated_at
        FROM expense_categories
    "#;

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<ExpenseCategory>, AppError> {
        let query = format!("{} WHERE id = $1", Self::SELECT);
        let category = sqlx::query_as::<_, ExpenseCategory>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(category)
    }

    pub async fn list(
        &self,
        filters: &ExpenseCategoryFilterDto,
    ) -> Result<PaginationResult<ExpenseCategory>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, name, description, \
             \"parentId\" as parent_id, \
             \"isActive\" as is_active, \
             \"budgetAmount\"::float8 as budget_amount, \
             \"budgetPeriod\" as budget_period, \
             \"createdBy\" as created_by, \
             \"updatedBy\" as updated_by, \
             \"createdAt\" as created_at, \
             \"updatedAt\" as updated_at \
             FROM expense_categories WHERE 1=1",
        );

        Self::apply_filters(&mut builder, filters);

        let sort_by = filters.sort_by.as_deref().unwrap_or("createdAt");
        let sort_col = match sort_by {
            "name" => "name",
            _ => "\"createdAt\"",
        };
        let sort_order = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        builder.push(format!(" ORDER BY {sort_col} {sort_order} LIMIT "));
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let items = builder
            .build_query_as::<ExpenseCategory>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM expense_categories WHERE 1=1");
        Self::apply_filters(&mut count_builder, filters);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(items, total.0, page, limit))
    }

    fn apply_filters(builder: &mut QueryBuilder<Postgres>, filters: &ExpenseCategoryFilterDto) {
        if let Some(name) = &filters.name {
            builder.push(" AND name ILIKE ");
            builder.push_bind(format!("%{name}%"));
        }
        if let Some(is_active) = filters.is_active {
            builder.push(" AND \"isActive\" = ");
            builder.push_bind(is_active);
        }
        if let Some(parent_id) = filters.parent_id {
            builder.push(" AND \"parentId\" = ");
            builder.push_bind(parent_id);
        }
    }

    pub async fn create(
        &self,
        dto: &CreateExpenseCategoryDto,
        created_by: Option<Uuid>,
    ) -> Result<ExpenseCategory, AppError> {
        let is_active = dto.is_active.unwrap_or(true);

        let category = sqlx::query_as::<_, ExpenseCategory>(
            r#"
            INSERT INTO expense_categories (
                id, name, description, "parentId", "isActive",
                "budgetAmount", "budgetPeriod", "createdBy", "updatedBy",
                "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $7, NOW(), NOW()
            )
            RETURNING id, name, description,
                      "parentId" as parent_id,
                      "isActive" as is_active,
                      "budgetAmount"::float8 as budget_amount,
                      "budgetPeriod" as budget_period,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(&dto.name)
        .bind(&dto.description)
        .bind(dto.parent_id)
        .bind(is_active)
        .bind(dto.budget_amount)
        .bind(&dto.budget_period)
        .bind(created_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(category)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdateExpenseCategoryDto,
        updated_by: Option<Uuid>,
    ) -> Result<ExpenseCategory, AppError> {
        let category = sqlx::query_as::<_, ExpenseCategory>(
            r#"
            UPDATE expense_categories SET
                name = COALESCE($2, name),
                description = COALESCE($3, description),
                "parentId" = COALESCE($4, "parentId"),
                "isActive" = COALESCE($5, "isActive"),
                "budgetAmount" = COALESCE($6, "budgetAmount"),
                "budgetPeriod" = COALESCE($7, "budgetPeriod"),
                "updatedBy" = $8,
                "updatedAt" = NOW()
            WHERE id = $1
            RETURNING id, name, description,
                      "parentId" as parent_id,
                      "isActive" as is_active,
                      "budgetAmount"::float8 as budget_amount,
                      "budgetPeriod" as budget_period,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(id)
        .bind(&dto.name)
        .bind(&dto.description)
        .bind(dto.parent_id)
        .bind(dto.is_active)
        .bind(dto.budget_amount)
        .bind(&dto.budget_period)
        .bind(updated_by)
        .fetch_optional(&self.pool)
        .await?;

        category
            .ok_or_else(|| AppError::NotFound(format!("Expense category with ID {id} not found")))
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM expense_categories WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "Expense category with ID {id} not found"
            )));
        }
        Ok(())
    }

    pub async fn name_exists(
        &self,
        name: &str,
        exclude_id: Option<Uuid>,
    ) -> Result<bool, AppError> {
        let exists: (bool,) = match exclude_id {
            Some(id) => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM expense_categories WHERE LOWER(name) = LOWER($1) AND id != $2)"#,
                )
                .bind(name)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM expense_categories WHERE LOWER(name) = LOWER($1))"#,
                )
                .bind(name)
                .fetch_one(&self.pool)
                .await?
            }
        };
        Ok(exists.0)
    }
}

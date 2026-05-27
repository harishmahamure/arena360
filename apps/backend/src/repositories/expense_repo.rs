use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CreateExpenseDto, Expense, ExpenseFilterDto, ExpenseSummaryDto, UpdateExpenseDto,
};

pub struct ExpenseRepository {
    pool: PgPool,
}

impl ExpenseRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id,
               "categoryId" as category_id,
               "vendorId" as vendor_id,
               amount::float8 as amount,
               "paymentMethod" as payment_method,
               "paymentAccount" as payment_account,
               description,
               "receiptUrl" as receipt_url,
               "expenseDate" as expense_date,
               "isRecurring" as is_recurring,
               "recurrencePattern" as recurrence_pattern,
               "nextRecurrenceDate" as next_recurrence_date,
               "approvalStatus" as approval_status,
               "approvedBy" as approved_by,
               "approvedAt" as approved_at,
               "rejectionReason" as rejection_reason,
               "shiftId" as shift_id,
               "cashRegisterEntryId" as cash_register_entry_id,
               "createdBy" as created_by,
               "updatedBy" as updated_by,
               "createdAt" as created_at,
               "updatedAt" as updated_at,
               "deletedAt" as deleted_at
        FROM expenses
    "#;

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Expense>, AppError> {
        let query = format!("{} WHERE id = $1 AND \"deletedAt\" IS NULL", Self::SELECT);
        let expense = sqlx::query_as::<_, Expense>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(expense)
    }

    pub async fn list(
        &self,
        filters: &ExpenseFilterDto,
    ) -> Result<PaginationResult<Expense>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, \
             \"categoryId\" as category_id, \
             \"vendorId\" as vendor_id, \
             amount::float8 as amount, \
             \"paymentMethod\" as payment_method, \
             \"paymentAccount\" as payment_account, \
             description, \
             \"receiptUrl\" as receipt_url, \
             \"expenseDate\" as expense_date, \
             \"isRecurring\" as is_recurring, \
             \"recurrencePattern\" as recurrence_pattern, \
             \"nextRecurrenceDate\" as next_recurrence_date, \
             \"approvalStatus\" as approval_status, \
             \"approvedBy\" as approved_by, \
             \"approvedAt\" as approved_at, \
             \"rejectionReason\" as rejection_reason, \
             \"shiftId\" as shift_id, \
             \"cashRegisterEntryId\" as cash_register_entry_id, \
             \"createdBy\" as created_by, \
             \"updatedBy\" as updated_by, \
             \"createdAt\" as created_at, \
             \"updatedAt\" as updated_at, \
             \"deletedAt\" as deleted_at \
             FROM expenses WHERE \"deletedAt\" IS NULL",
        );

        Self::apply_filters(&mut builder, filters);

        let sort_by = filters.sort_by.as_deref().unwrap_or("expenseDate");
        let sort_col = match sort_by {
            "amount" => "amount",
            "createdAt" => "\"createdAt\"",
            "approvalStatus" => "\"approvalStatus\"",
            _ => "\"expenseDate\"",
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
            .build_query_as::<Expense>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM expenses WHERE \"deletedAt\" IS NULL");
        Self::apply_filters(&mut count_builder, filters);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(items, total.0, page, limit))
    }

    fn apply_filters(builder: &mut QueryBuilder<Postgres>, filters: &ExpenseFilterDto) {
        if let Some(category_id) = filters.category_id {
            builder.push(" AND \"categoryId\" = ");
            builder.push_bind(category_id);
        }
        if let Some(vendor_id) = filters.vendor_id {
            builder.push(" AND \"vendorId\" = ");
            builder.push_bind(vendor_id);
        }
        if let Some(status) = &filters.approval_status {
            builder.push(" AND \"approvalStatus\" = ");
            builder.push_bind(status.clone());
        }
        if let Some(method) = &filters.payment_method {
            builder.push(" AND \"paymentMethod\" = ");
            builder.push_bind(method.clone());
        }
        if let Some(date_from) = filters.date_from {
            builder.push(" AND \"expenseDate\" >= ");
            builder.push_bind(date_from);
        }
        if let Some(date_to) = filters.date_to {
            builder.push(" AND \"expenseDate\" <= ");
            builder.push_bind(date_to);
        }
        if let Some(min_amount) = filters.min_amount {
            builder.push(" AND amount >= ");
            builder.push_bind(min_amount);
        }
        if let Some(max_amount) = filters.max_amount {
            builder.push(" AND amount <= ");
            builder.push_bind(max_amount);
        }
        if let Some(shift_id) = filters.shift_id {
            builder.push(" AND \"shiftId\" = ");
            builder.push_bind(shift_id);
        }
        if let Some(is_recurring) = filters.is_recurring {
            builder.push(" AND \"isRecurring\" = ");
            builder.push_bind(is_recurring);
        }
    }

    pub async fn create(
        &self,
        dto: &CreateExpenseDto,
        created_by: Option<Uuid>,
    ) -> Result<Expense, AppError> {
        let is_recurring = dto.is_recurring.unwrap_or(false);

        let expense = sqlx::query_as::<_, Expense>(
            r#"
            INSERT INTO expenses (
                id, "categoryId", "vendorId", amount, "paymentMethod",
                "paymentAccount", description, "receiptUrl", "expenseDate",
                "isRecurring", "recurrencePattern", "approvalStatus",
                "shiftId", "createdBy", "updatedBy", "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7,
                COALESCE($8, NOW()), $9, $10, 'pending',
                $11, $12, $12, NOW(), NOW()
            )
            RETURNING id,
                      "categoryId" as category_id,
                      "vendorId" as vendor_id,
                      amount::float8 as amount,
                      "paymentMethod" as payment_method,
                      "paymentAccount" as payment_account,
                      description,
                      "receiptUrl" as receipt_url,
                      "expenseDate" as expense_date,
                      "isRecurring" as is_recurring,
                      "recurrencePattern" as recurrence_pattern,
                      "nextRecurrenceDate" as next_recurrence_date,
                      "approvalStatus" as approval_status,
                      "approvedBy" as approved_by,
                      "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason,
                      "shiftId" as shift_id,
                      "cashRegisterEntryId" as cash_register_entry_id,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(dto.category_id)
        .bind(dto.vendor_id)
        .bind(dto.amount)
        .bind(&dto.payment_method)
        .bind(&dto.payment_account)
        .bind(&dto.description)
        .bind(&dto.receipt_url)
        .bind(dto.expense_date)
        .bind(is_recurring)
        .bind(&dto.recurrence_pattern)
        .bind(dto.shift_id)
        .bind(created_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(expense)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdateExpenseDto,
        updated_by: Option<Uuid>,
    ) -> Result<Expense, AppError> {
        let expense = sqlx::query_as::<_, Expense>(
            r#"
            UPDATE expenses SET
                "categoryId" = COALESCE($2, "categoryId"),
                "vendorId" = COALESCE($3, "vendorId"),
                amount = COALESCE($4, amount),
                "paymentMethod" = COALESCE($5, "paymentMethod"),
                "paymentAccount" = COALESCE($6, "paymentAccount"),
                description = COALESCE($7, description),
                "receiptUrl" = COALESCE($8, "receiptUrl"),
                "expenseDate" = COALESCE($9, "expenseDate"),
                "isRecurring" = COALESCE($10, "isRecurring"),
                "recurrencePattern" = COALESCE($11, "recurrencePattern"),
                "shiftId" = COALESCE($12, "shiftId"),
                "updatedBy" = $13,
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id,
                      "categoryId" as category_id,
                      "vendorId" as vendor_id,
                      amount::float8 as amount,
                      "paymentMethod" as payment_method,
                      "paymentAccount" as payment_account,
                      description,
                      "receiptUrl" as receipt_url,
                      "expenseDate" as expense_date,
                      "isRecurring" as is_recurring,
                      "recurrencePattern" as recurrence_pattern,
                      "nextRecurrenceDate" as next_recurrence_date,
                      "approvalStatus" as approval_status,
                      "approvedBy" as approved_by,
                      "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason,
                      "shiftId" as shift_id,
                      "cashRegisterEntryId" as cash_register_entry_id,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .bind(dto.category_id)
        .bind(dto.vendor_id)
        .bind(dto.amount)
        .bind(&dto.payment_method)
        .bind(&dto.payment_account)
        .bind(&dto.description)
        .bind(&dto.receipt_url)
        .bind(dto.expense_date)
        .bind(dto.is_recurring)
        .bind(&dto.recurrence_pattern)
        .bind(dto.shift_id)
        .bind(updated_by)
        .fetch_optional(&self.pool)
        .await?;

        expense.ok_or_else(|| AppError::NotFound(format!("Expense with ID {id} not found")))
    }

    pub async fn soft_delete(&self, id: Uuid) -> Result<Expense, AppError> {
        let expense = sqlx::query_as::<_, Expense>(
            r#"
            UPDATE expenses SET "deletedAt" = NOW(), "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id,
                      "categoryId" as category_id,
                      "vendorId" as vendor_id,
                      amount::float8 as amount,
                      "paymentMethod" as payment_method,
                      "paymentAccount" as payment_account,
                      description,
                      "receiptUrl" as receipt_url,
                      "expenseDate" as expense_date,
                      "isRecurring" as is_recurring,
                      "recurrencePattern" as recurrence_pattern,
                      "nextRecurrenceDate" as next_recurrence_date,
                      "approvalStatus" as approval_status,
                      "approvedBy" as approved_by,
                      "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason,
                      "shiftId" as shift_id,
                      "cashRegisterEntryId" as cash_register_entry_id,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        expense.ok_or_else(|| AppError::NotFound(format!("Expense with ID {id} not found")))
    }

    pub async fn approve(&self, id: Uuid, approved_by: Uuid) -> Result<Expense, AppError> {
        let expense = sqlx::query_as::<_, Expense>(
            r#"
            UPDATE expenses SET
                "approvalStatus" = 'approved',
                "approvedBy" = $2,
                "approvedAt" = NOW(),
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id,
                      "categoryId" as category_id,
                      "vendorId" as vendor_id,
                      amount::float8 as amount,
                      "paymentMethod" as payment_method,
                      "paymentAccount" as payment_account,
                      description,
                      "receiptUrl" as receipt_url,
                      "expenseDate" as expense_date,
                      "isRecurring" as is_recurring,
                      "recurrencePattern" as recurrence_pattern,
                      "nextRecurrenceDate" as next_recurrence_date,
                      "approvalStatus" as approval_status,
                      "approvedBy" as approved_by,
                      "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason,
                      "shiftId" as shift_id,
                      "cashRegisterEntryId" as cash_register_entry_id,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .bind(approved_by)
        .fetch_optional(&self.pool)
        .await?;

        expense.ok_or_else(|| AppError::NotFound(format!("Expense with ID {id} not found")))
    }

    pub async fn reject(
        &self,
        id: Uuid,
        rejection_reason: &str,
        rejected_by: Uuid,
    ) -> Result<Expense, AppError> {
        let expense = sqlx::query_as::<_, Expense>(
            r#"
            UPDATE expenses SET
                "approvalStatus" = 'rejected',
                "rejectionReason" = $2,
                "approvedBy" = $3,
                "approvedAt" = NOW(),
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id,
                      "categoryId" as category_id,
                      "vendorId" as vendor_id,
                      amount::float8 as amount,
                      "paymentMethod" as payment_method,
                      "paymentAccount" as payment_account,
                      description,
                      "receiptUrl" as receipt_url,
                      "expenseDate" as expense_date,
                      "isRecurring" as is_recurring,
                      "recurrencePattern" as recurrence_pattern,
                      "nextRecurrenceDate" as next_recurrence_date,
                      "approvalStatus" as approval_status,
                      "approvedBy" as approved_by,
                      "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason,
                      "shiftId" as shift_id,
                      "cashRegisterEntryId" as cash_register_entry_id,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .bind(rejection_reason)
        .bind(rejected_by)
        .fetch_optional(&self.pool)
        .await?;

        expense.ok_or_else(|| AppError::NotFound(format!("Expense with ID {id} not found")))
    }

    pub async fn set_cash_register_entry_id(
        &self,
        expense_id: Uuid,
        entry_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"UPDATE expenses SET "cashRegisterEntryId" = $1, "updatedAt" = NOW() WHERE id = $2"#,
        )
        .bind(entry_id)
        .bind(expense_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_summary_by_category(&self) -> Result<Vec<ExpenseSummaryDto>, AppError> {
        let summaries = sqlx::query_as::<_, ExpenseSummaryDto>(
            r#"
            SELECT
                ec.name as category_name,
                ec."budgetAmount"::float8 as budget_amount,
                ec."budgetPeriod" as budget_period,
                COALESCE(SUM(e.amount)::float8, 0) as total_spent,
                CASE
                    WHEN ec."budgetAmount" IS NOT NULL
                    THEN (ec."budgetAmount" - COALESCE(SUM(e.amount), 0))::float8
                    ELSE NULL
                END as remaining_budget,
                COUNT(e.id) as expense_count
            FROM expense_categories ec
            LEFT JOIN expenses e ON e."categoryId" = ec.id
                AND e."deletedAt" IS NULL
                AND e."approvalStatus" = 'approved'
            WHERE ec."isActive" = true
            GROUP BY ec.id, ec.name, ec."budgetAmount", ec."budgetPeriod"
            ORDER BY total_spent DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(summaries)
    }
}

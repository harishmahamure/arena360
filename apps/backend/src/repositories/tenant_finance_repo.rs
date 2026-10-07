//! Fenced tenant-local shifts, cash, and expense workflows.
use super::tenant_back_office::{event, money, money_f64, now, write};
use crate::{
    dto::PaginationResult,
    error::AppError,
    models::*,
    tenancy::{format_sqlite_timestamp, TenantDb},
};
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Sqlite, SqliteConnection};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct TenantShiftRepository {
    db: Arc<TenantDb>,
}
impl TenantShiftRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    const SELECT: &'static str = r#"
        SELECT unhex(replace(id,'-','')) AS id,
               unhex(replace(user_id,'-','')) AS user_id,
               clock_in as clock_in,
               clock_out as clock_out,
               notes,
               CASE WHEN status='closed' THEN CASE WHEN close_kind='force' THEN 'force_closed' ELSE 'completed' END ELSE status END AS status,
               unhex(replace(created_by,'-','')) AS created_by,
               unhex(replace(updated_by,'-','')) AS updated_by,
               created_at as created_at,
               updated_at as updated_at
        FROM shifts
    "#;
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Shift>, AppError> {
        let query = format!("{} WHERE id = $1", Self::SELECT);
        let shift = sqlx::query_as::<_, Shift>(&query)
            .bind(id.to_string())
            .fetch_optional(&self.db.read_pool()?)
            .await?;
        Ok(shift)
    }
    pub async fn location_id(&self,id:Uuid)->Result<Uuid,AppError> {
        sqlx::query_scalar("SELECT unhex(replace(location_id,'-','')) FROM shifts WHERE id=?")
            .bind(id.to_string()).fetch_optional(&self.db.read_pool()?).await?
            .ok_or_else(||AppError::NotFound("Shift not found".into()))
    }
    pub async fn find_active_by_user(&self, user_id: Uuid) -> Result<Option<Shift>, AppError> {
        let query = format!("{} WHERE user_id = $1 AND status = 'active'", Self::SELECT);
        let shift = sqlx::query_as::<_, Shift>(&query)
            .bind(user_id.to_string())
            .fetch_optional(&self.db.read_pool()?)
            .await?;
        Ok(shift)
    }
    pub async fn list(
        &self,
        filters: &ShiftFilterDto,
    ) -> Result<PaginationResult<Shift>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1).saturating_mul(limit);

        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            r#"SELECT unhex(replace(id,'-','')) AS id,
                      unhex(replace(user_id,'-','')) AS user_id,
                      clock_in as clock_in,
                      clock_out as clock_out,
                      notes,
                      CASE WHEN status='closed' THEN CASE WHEN close_kind='force' THEN 'force_closed' ELSE 'completed' END ELSE status END AS status,
                      unhex(replace(created_by,'-','')) AS created_by,
                      unhex(replace(updated_by,'-','')) AS updated_by,
                      created_at as created_at,
                      updated_at as updated_at
               FROM shifts WHERE 1=1"#,
        );

        Self::apply_filters(&mut builder, filters)?;

        let sort_by = filters.sort_by.as_deref().unwrap_or("clockIn");
        let sort_col = match sort_by {
            "clockOut" => "clock_out",
            "status" => "status",
            "createdAt" => "created_at",
            _ => "clock_in",
        };
        let sort_order = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        builder.push(format!(
            " ORDER BY {sort_col} {sort_order}, id {sort_order} LIMIT "
        ));
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let rows = builder
            .build_query_as::<Shift>()
            .fetch_all(&self.db.read_pool()?)
            .await?;

        let mut count_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT COUNT(*) FROM shifts WHERE 1=1");
        Self::apply_filters(&mut count_builder, filters)?;

        let total: (i64,) = count_builder
            .build_query_as()
            .fetch_one(&self.db.read_pool()?)
            .await?;

        Ok(PaginationResult::new(rows, total.0, page, limit))
    }
    fn apply_filters(
        builder: &mut QueryBuilder<Sqlite>,
        filters: &ShiftFilterDto,
    ) -> Result<(), AppError> {
        if let Some(user_id) = filters.user_id {
            builder.push(" AND user_id = ");
            builder.push_bind(user_id.to_string());
        }
        if let Some(status) = &filters.status {
            builder.push(" AND CASE WHEN status='closed' THEN CASE WHEN close_kind='force' THEN 'force_closed' ELSE 'completed' END ELSE status END = ");
            builder.push_bind(status.clone());
        }
        if let Some(from) = filters.clock_in_from {
            builder.push(" AND clock_in >= ");
            builder.push_bind(
                format_sqlite_timestamp(&from).map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
        if let Some(to) = filters.clock_in_to {
            builder.push(" AND clock_in <= ");
            builder.push_bind(
                format_sqlite_timestamp(&to).map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct TenantCashRegisterRepository {
    db: Arc<TenantDb>,
}
impl TenantCashRegisterRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    const SELECT: &'static str = r#"
        SELECT unhex(replace(id,'-','')) AS id,
               unhex(replace(shift_id,'-','')) AS shift_id,
               unhex(replace(opened_by,'-','')) AS opened_by,
               unhex(replace(closed_by,'-','')) AS closed_by,
               opening_balance/10000.0 AS opening_balance,
               opening_denominations as opening_denominations,
               closing_balance/10000.0 AS closing_balance,
               closing_denominations as closing_denominations,
               expected_closing/10000.0 AS expected_closing,
               variance/10000.0 AS variance,
               status,
               notes,
               unhex(replace(reconciled_by,'-','')) AS reconciled_by,
               reconciled_at as reconciled_at,
               reconciliation_notes as reconciliation_notes,
               unhex(replace(created_by,'-','')) AS created_by,
               unhex(replace(updated_by,'-','')) AS updated_by,
               created_at as created_at,
               updated_at as updated_at
        FROM cash_registers
    "#;
    const ENTRY_AGGREGATES_JOIN: &'static str = r#"
               LEFT JOIN (
                   SELECT e.cash_register_id,
                          SUM(e.amount) FILTER (WHERE e.entry_type = 'cash_in') AS total_cash_in,
                          SUM(e.amount) FILTER (WHERE e.entry_type = 'cash_out') AS total_cash_out,
                          SUM(e.amount) FILTER (
                              WHERE e.entry_type = 'cash_out'
                                AND e.reference_type = 'cash_deposit'
                                AND d.status = 'approved'
                          ) AS total_deposited
                   FROM cash_register_entries e
                   LEFT JOIN cash_deposits d ON d.id = e.reference_id
                   GROUP BY e.cash_register_id
               ) agg ON agg.cash_register_id = cr.id"#;
    const SELECT_WITH_TOTALS: &'static str = r#"
        SELECT unhex(replace(cr.id,'-','')) AS id,
               unhex(replace(cr.shift_id,'-','')) AS shift_id,
               unhex(replace(cr.opened_by,'-','')) AS opened_by,
               unhex(replace(cr.closed_by,'-','')) AS closed_by,
               cr.opening_balance/10000.0 AS opening_balance,
               cr.opening_denominations as opening_denominations,
               cr.closing_balance/10000.0 AS closing_balance,
               cr.closing_denominations as closing_denominations,
               cr.expected_closing/10000.0 AS expected_closing,
               cr.variance/10000.0 AS variance,
               cr.status,
               cr.notes,
               unhex(replace(cr.reconciled_by,'-','')) AS reconciled_by,
               cr.reconciled_at as reconciled_at,
               cr.reconciliation_notes as reconciliation_notes,
               unhex(replace(cr.created_by,'-','')) AS created_by,
               unhex(replace(cr.updated_by,'-','')) AS updated_by,
               cr.created_at as created_at,
               cr.updated_at as updated_at,
               COALESCE(agg.total_cash_in,0)/10000.0 AS total_cash_in,
               COALESCE(agg.total_cash_out,0)/10000.0 AS total_cash_out,
               COALESCE(agg.total_deposited,0)/10000.0 AS total_deposited
        FROM cash_registers cr
    "#;
    const ENTRY_SELECT: &'static str = r#"
        SELECT unhex(replace(id,'-','')) AS id,
               unhex(replace(cash_register_id,'-','')) AS cash_register_id,
               entry_type as entry_type,
               amount/10000.0 AS amount,
               reason,
               unhex(replace(reference_id,'-','')) AS reference_id,
               reference_type as reference_type,
               unhex(replace(created_by,'-','')) AS created_by,
               created_at as created_at
        FROM cash_register_entries
    "#;
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<CashRegister>, AppError> {
        let query = format!(
            "{}{} WHERE cr.id = $1",
            Self::SELECT_WITH_TOTALS,
            Self::ENTRY_AGGREGATES_JOIN
        );
        let register = sqlx::query_as::<_, CashRegister>(&query)
            .bind(id.to_string())
            .fetch_optional(&self.db.read_pool()?)
            .await?;
        Ok(register)
    }
    pub async fn find_by_shift(&self, shift_id: Uuid) -> Result<Option<CashRegister>, AppError> {
        let query = format!(
            "{} WHERE shift_id = $1 ORDER BY (status='open') DESC,created_at DESC,id DESC LIMIT 1",
            Self::SELECT
        );
        let register = sqlx::query_as::<_, CashRegister>(&query)
            .bind(shift_id.to_string())
            .fetch_optional(&self.db.read_pool()?)
            .await?;
        Ok(register)
    }
    pub async fn list(
        &self,
        filters: &CashRegisterFilterDto,
    ) -> Result<PaginationResult<CashRegister>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1).saturating_mul(limit);

        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(format!(
            "{}{} WHERE 1=1",
            Self::SELECT_WITH_TOTALS,
            Self::ENTRY_AGGREGATES_JOIN
        ));

        Self::apply_filters(&mut builder, filters)?;

        let sort_by = filters.sort_by.as_deref().unwrap_or("createdAt");
        let sort_col = match sort_by {
            "openingBalance" => "cr.opening_balance",
            "status" => "cr.status",
            "updatedAt" => "cr.updated_at",
            _ => "cr.created_at",
        };
        let sort_order = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        builder.push(format!(
            " ORDER BY {sort_col} {sort_order}, cr.id {sort_order} LIMIT "
        ));
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let rows = builder
            .build_query_as::<CashRegister>()
            .fetch_all(&self.db.read_pool()?)
            .await?;

        let mut count_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT COUNT(*) FROM cash_registers cr WHERE 1=1");
        Self::apply_filters(&mut count_builder, filters)?;

        let total: (i64,) = count_builder
            .build_query_as()
            .fetch_one(&self.db.read_pool()?)
            .await?;

        Ok(PaginationResult::new(rows, total.0, page, limit))
    }
    fn apply_filters(
        builder: &mut QueryBuilder<Sqlite>,
        filters: &CashRegisterFilterDto,
    ) -> Result<(), AppError> {
        if let Some(shift_id) = filters.shift_id {
            builder.push(" AND cr.shift_id = ");
            builder.push_bind(shift_id.to_string());
        }
        if let Some(ref status) = filters.status {
            builder.push(" AND cr.status = ");
            builder.push_bind(status.clone());
        }
        if let Some(opened_by) = filters.opened_by {
            builder.push(" AND cr.opened_by = ");
            builder.push_bind(opened_by.to_string());
        }
        Ok(())
    }
    pub async fn find_last_closed_by_user(
        &self,
        user_id: Uuid,
    ) -> Result<Option<CashRegister>, AppError> {
        let register = sqlx::query_as::<_, CashRegister>(
            r#"
            SELECT unhex(replace(cr.id,'-','')) AS id,
                   unhex(replace(cr.shift_id,'-','')) AS shift_id,
                   unhex(replace(cr.opened_by,'-','')) AS opened_by,
                   unhex(replace(cr.closed_by,'-','')) AS closed_by,
                   cr.opening_balance/10000.0 AS opening_balance,
                   cr.opening_denominations as opening_denominations,
                   cr.closing_balance/10000.0 AS closing_balance,
                   cr.closing_denominations as closing_denominations,
                   cr.expected_closing/10000.0 AS expected_closing,
                   cr.variance/10000.0 AS variance,
                   cr.status,
                   cr.notes,
                   unhex(replace(cr.reconciled_by,'-','')) AS reconciled_by,
                   cr.reconciled_at as reconciled_at,
                   cr.reconciliation_notes as reconciliation_notes,
                   unhex(replace(cr.created_by,'-','')) AS created_by,
                   unhex(replace(cr.updated_by,'-','')) AS updated_by,
                   cr.created_at as created_at,
                   cr.updated_at as updated_at
            FROM cash_registers cr
            INNER JOIN shifts s ON s.id = cr.shift_id
            WHERE s.user_id = $1 AND cr.status IN ('closed', 'reconciled')
            ORDER BY cr.updated_at DESC,cr.id DESC
            LIMIT 1
            "#,
        )
        .bind(user_id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?;
        Ok(register)
    }
    pub async fn find_last_closed_register(&self) -> Result<Option<CashRegister>, AppError> {
        let query = format!(
            r#"{base} WHERE status IN ('closed', 'reconciled') ORDER BY updated_at DESC,id DESC LIMIT 1"#,
            base = Self::SELECT
        );
        let register = sqlx::query_as::<_, CashRegister>(&query)
            .fetch_optional(&self.db.read_pool()?)
            .await?;
        Ok(register)
    }
    pub async fn find_last_closed_register_for(
        &self,
        venue_id: Uuid,
    ) -> Result<Option<CashRegister>, AppError> {
        let query = format!(
            r#"{} WHERE id = (
            SELECT c.id FROM cash_registers c JOIN shifts s ON s.id=c.shift_id
            WHERE s.location_id=$1 AND c.status IN ('closed','reconciled')
            ORDER BY c.updated_at DESC,c.id DESC LIMIT 1
        )"#,
            Self::SELECT
        );
        Ok(sqlx::query_as::<_, CashRegister>(&query)
            .bind(venue_id.to_string())
            .fetch_optional(&self.db.read_pool()?)
            .await?)
    }
    pub async fn list_entries(
        &self,
        register_id: Uuid,
    ) -> Result<Vec<CashRegisterEntry>, AppError> {
        let query = format!(
            "{} WHERE cash_register_id = $1 ORDER BY created_at ASC",
            Self::ENTRY_SELECT
        );
        let entries = sqlx::query_as::<_, CashRegisterEntry>(&query)
            .bind(register_id.to_string())
            .fetch_all(&self.db.read_pool()?)
            .await?;
        Ok(entries)
    }
}

#[derive(Clone)]
pub struct TenantCashDepositRepository {
    db: Arc<TenantDb>,
}
impl TenantCashDepositRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    const SELECT: &'static str = r#"
        SELECT unhex(replace(id,'-','')) AS id,
               unhex(replace(cash_register_id,'-','')) AS cash_register_id,
               unhex(replace(shift_id,'-','')) AS shift_id,
               unhex(replace(initiated_by,'-','')) AS initiated_by,
               unhex(replace(approved_by,'-','')) AS approved_by,
               amount/10000.0 AS amount,
               denominations,
               deposit_type as deposit_type,
               status,
               approved_at as approved_at,
               rejection_reason as rejection_reason,
               notes,
               created_at as created_at,
               updated_at as updated_at
        FROM cash_deposits
    "#;
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<CashDeposit>, AppError> {
        let query = format!("{} WHERE id = $1", Self::SELECT);
        let deposit = sqlx::query_as::<_, CashDeposit>(&query)
            .bind(id.to_string())
            .fetch_optional(&self.db.read_pool()?)
            .await?;
        Ok(deposit)
    }
    pub async fn list(
        &self,
        filters: &CashDepositFilterDto,
    ) -> Result<PaginationResult<CashDeposit>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1).saturating_mul(limit);

        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            r#"SELECT unhex(replace(id,'-','')) AS id,
                      unhex(replace(cash_register_id,'-','')) AS cash_register_id,
                      unhex(replace(shift_id,'-','')) AS shift_id,
                      unhex(replace(initiated_by,'-','')) AS initiated_by,
                      unhex(replace(approved_by,'-','')) AS approved_by,
                      amount/10000.0 AS amount,
                      denominations,
                      deposit_type as deposit_type,
                      status,
                      approved_at as approved_at,
                      rejection_reason as rejection_reason,
                      notes,
                      created_at as created_at,
                      updated_at as updated_at
               FROM cash_deposits WHERE 1=1"#,
        );

        Self::apply_filters(&mut builder, filters)?;

        let sort_by = filters.sort_by.as_deref().unwrap_or("createdAt");
        let sort_col = match sort_by {
            "amount" => "amount",
            "status" => "status",
            _ => "created_at",
        };
        let sort_order = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        builder.push(format!(
            " ORDER BY {sort_col} {sort_order}, id {sort_order} LIMIT "
        ));
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let rows = builder
            .build_query_as::<CashDeposit>()
            .fetch_all(&self.db.read_pool()?)
            .await?;

        let mut count_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT COUNT(*) FROM cash_deposits WHERE 1=1");
        Self::apply_filters(&mut count_builder, filters)?;

        let total: (i64,) = count_builder
            .build_query_as()
            .fetch_one(&self.db.read_pool()?)
            .await?;

        Ok(PaginationResult::new(rows, total.0, page, limit))
    }
    fn apply_filters(
        builder: &mut QueryBuilder<Sqlite>,
        filters: &CashDepositFilterDto,
    ) -> Result<(), AppError> {
        if let Some(shift_id) = filters.shift_id {
            builder.push(" AND shift_id = ");
            builder.push_bind(shift_id.to_string());
        }
        if let Some(cash_register_id) = filters.cash_register_id {
            builder.push(" AND cash_register_id = ");
            builder.push_bind(cash_register_id.to_string());
        }
        if let Some(status) = &filters.status {
            builder.push(" AND status = ");
            builder.push_bind(status.clone());
        }
        if let Some(initiated_by) = filters.initiated_by {
            builder.push(" AND initiated_by = ");
            builder.push_bind(initiated_by.to_string());
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct TenantExpenseRepository {
    db: Arc<TenantDb>,
}
impl TenantExpenseRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    const SELECT: &'static str = r#"
        SELECT unhex(replace(id,'-','')) AS id,
               unhex(replace(category_id,'-','')) AS category_id,
               unhex(replace(vendor_id,'-','')) AS vendor_id,
               amount/10000.0 AS amount,
               payment_method as payment_method,
               payment_account as payment_account,
               description,
               receipt_url as receipt_url,
               expense_date as expense_date,
               is_recurring as is_recurring,
               recurrence_pattern as recurrence_pattern,
               next_recurrence_date as next_recurrence_date,
               approval_status as approval_status,
               unhex(replace(approved_by,'-','')) AS approved_by,
               approved_at as approved_at,
               rejection_reason as rejection_reason,
               unhex(replace(shift_id,'-','')) AS shift_id,
               unhex(replace(cash_register_entry_id,'-','')) AS cash_register_entry_id,
               unhex(replace(created_by,'-','')) AS created_by,
               unhex(replace(updated_by,'-','')) AS updated_by,
               created_at as created_at,
               updated_at as updated_at,
               deleted_at as deleted_at
        FROM expenses
    "#;
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Expense>, AppError> {
        let query = format!("{} WHERE id = $1 AND deleted_at IS NULL", Self::SELECT);
        let expense = sqlx::query_as::<_, Expense>(&query)
            .bind(id.to_string())
            .fetch_optional(&self.db.read_pool()?)
            .await?;
        Ok(expense)
    }
    pub async fn list(
        &self,
        filters: &ExpenseFilterDto,
    ) -> Result<PaginationResult<Expense>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1).saturating_mul(limit);

        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "SELECT unhex(replace(id,'-','')) AS id, \
             unhex(replace(category_id,'-','')) AS category_id, \
             unhex(replace(vendor_id,'-','')) AS vendor_id, \
             amount/10000.0 AS amount, \
             payment_method as payment_method, \
             payment_account as payment_account, \
             description, \
             receipt_url as receipt_url, \
             expense_date as expense_date, \
             is_recurring as is_recurring, \
             recurrence_pattern as recurrence_pattern, \
             next_recurrence_date as next_recurrence_date, \
             approval_status as approval_status, \
             unhex(replace(approved_by,'-','')) AS approved_by, \
             approved_at as approved_at, \
             rejection_reason as rejection_reason, \
             unhex(replace(shift_id,'-','')) AS shift_id, \
             unhex(replace(cash_register_entry_id,'-','')) AS cash_register_entry_id, \
             unhex(replace(created_by,'-','')) AS created_by, \
             unhex(replace(updated_by,'-','')) AS updated_by, \
             created_at as created_at, \
             updated_at as updated_at, \
             deleted_at as deleted_at \
             FROM expenses WHERE deleted_at IS NULL",
        );

        Self::apply_filters(&mut builder, filters)?;

        let sort_by = filters.sort_by.as_deref().unwrap_or("expenseDate");
        let sort_col = match sort_by {
            "amount" => "amount",
            "createdAt" => "created_at",
            "approvalStatus" => "approval_status",
            _ => "expense_date",
        };
        let sort_order = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        builder.push(format!(
            " ORDER BY {sort_col} {sort_order}, id {sort_order} LIMIT "
        ));
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let items = builder
            .build_query_as::<Expense>()
            .fetch_all(&self.db.read_pool()?)
            .await?;

        let mut count_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT COUNT(*) FROM expenses WHERE deleted_at IS NULL");
        Self::apply_filters(&mut count_builder, filters)?;

        let total: (i64,) = count_builder
            .build_query_as()
            .fetch_one(&self.db.read_pool()?)
            .await?;

        Ok(PaginationResult::new(items, total.0, page, limit))
    }
    fn apply_filters(
        builder: &mut QueryBuilder<Sqlite>,
        filters: &ExpenseFilterDto,
    ) -> Result<(), AppError> {
        if let Some(category_id) = filters.category_id {
            builder.push(" AND category_id = ");
            builder.push_bind(category_id.to_string());
        }
        if let Some(vendor_id) = filters.vendor_id {
            builder.push(" AND vendor_id = ");
            builder.push_bind(vendor_id.to_string());
        }
        if let Some(status) = &filters.approval_status {
            builder.push(" AND approval_status = ");
            builder.push_bind(status.clone());
        }
        if let Some(method) = &filters.payment_method {
            builder.push(" AND payment_method = ");
            builder.push_bind(method.clone());
        }
        if let Some(date_from) = filters.date_from {
            builder.push(" AND expense_date >= ");
            builder.push_bind(
                format_sqlite_timestamp(&date_from)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
        if let Some(date_to) = filters.date_to {
            builder.push(" AND expense_date <= ");
            builder.push_bind(
                format_sqlite_timestamp(&date_to)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
        if let Some(min_amount) = filters.min_amount {
            builder.push(" AND amount >= ");
            builder.push_bind(money(min_amount)?);
        }
        if let Some(max_amount) = filters.max_amount {
            builder.push(" AND amount <= ");
            builder.push_bind(money(max_amount)?);
        }
        if let Some(shift_id) = filters.shift_id {
            builder.push(" AND shift_id = ");
            builder.push_bind(shift_id.to_string());
        }
        if let Some(is_recurring) = filters.is_recurring {
            builder.push(" AND is_recurring = ");
            builder.push_bind(is_recurring);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct TenantExpenseCategoryRepository {
    db: Arc<TenantDb>,
}
impl TenantExpenseCategoryRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    const SELECT: &'static str = r#"
        SELECT unhex(replace(id,'-','')) AS id, name, description,
               unhex(replace(parent_id,'-','')) AS parent_id,
               is_active as is_active,
               budget_amount/10000.0 AS budget_amount,
               budget_period as budget_period,
               unhex(replace(created_by,'-','')) AS created_by,
               unhex(replace(updated_by,'-','')) AS updated_by,
               created_at as created_at,
               updated_at as updated_at
        FROM expense_categories
    "#;
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<ExpenseCategory>, AppError> {
        let query = format!("{} WHERE id = $1", Self::SELECT);
        let category = sqlx::query_as::<_, ExpenseCategory>(&query)
            .bind(id.to_string())
            .fetch_optional(&self.db.read_pool()?)
            .await?;
        Ok(category)
    }
    pub async fn list(
        &self,
        filters: &ExpenseCategoryFilterDto,
    ) -> Result<PaginationResult<ExpenseCategory>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1).saturating_mul(limit);

        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "SELECT unhex(replace(id,'-','')) AS id, name, description, \
             unhex(replace(parent_id,'-','')) AS parent_id, \
             is_active as is_active, \
             budget_amount/10000.0 AS budget_amount, \
             budget_period as budget_period, \
             unhex(replace(created_by,'-','')) AS created_by, \
             unhex(replace(updated_by,'-','')) AS updated_by, \
             created_at as created_at, \
             updated_at as updated_at \
             FROM expense_categories WHERE 1=1",
        );

        Self::apply_filters(&mut builder, filters)?;

        let sort_by = filters.sort_by.as_deref().unwrap_or("createdAt");
        let sort_col = match sort_by {
            "name" => "name",
            _ => "created_at",
        };
        let sort_order = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        builder.push(format!(
            " ORDER BY {sort_col} {sort_order}, id {sort_order} LIMIT "
        ));
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let items = builder
            .build_query_as::<ExpenseCategory>()
            .fetch_all(&self.db.read_pool()?)
            .await?;

        let mut count_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT COUNT(*) FROM expense_categories WHERE 1=1");
        Self::apply_filters(&mut count_builder, filters)?;

        let total: (i64,) = count_builder
            .build_query_as()
            .fetch_one(&self.db.read_pool()?)
            .await?;

        Ok(PaginationResult::new(items, total.0, page, limit))
    }
    fn apply_filters(
        builder: &mut QueryBuilder<Sqlite>,
        filters: &ExpenseCategoryFilterDto,
    ) -> Result<(), AppError> {
        if let Some(name) = &filters.name {
            builder.push(" AND name LIKE ");
            builder.push_bind(format!("%{name}%"));
        }
        if let Some(is_active) = filters.is_active {
            builder.push(" AND is_active = ");
            builder.push_bind(is_active);
        }
        if let Some(parent_id) = filters.parent_id {
            builder.push(" AND parent_id = ");
            builder.push_bind(parent_id.to_string());
        }
        Ok(())
    }
}

fn nonnegative(value: f64) -> Result<i64, AppError> {
    let value = money(value)?;
    if value < 0 {
        return Err(AppError::BadRequest("Amount cannot be negative".into()));
    }
    Ok(value)
}
fn positive(value: f64) -> Result<i64, AppError> {
    let value = nonnegative(value)?;
    if value == 0 {
        return Err(AppError::BadRequest(
            "Amount must be greater than zero".into(),
        ));
    }
    Ok(value)
}
fn checked_add(a: i64, b: i64) -> Result<i64, AppError> {
    a.checked_add(b)
        .ok_or_else(|| AppError::BadRequest("Cash amount overflow".into()))
}
fn checked_sub(a: i64, b: i64) -> Result<i64, AppError> {
    a.checked_sub(b)
        .ok_or_else(|| AppError::BadRequest("Cash amount overflow".into()))
}
fn json_option(value: Option<Value>) -> Result<Option<String>, AppError> {
    value
        .map(|x| serde_json::to_string(&x).map_err(|e| AppError::BadRequest(e.to_string())))
        .transpose()
}
fn date(value: Option<chrono::DateTime<chrono::Utc>>) -> Result<Option<String>, AppError> {
    value
        .map(|x| format_sqlite_timestamp(&x).map_err(|e| AppError::BadRequest(e.to_string())))
        .transpose()
}
async fn shift_location(c: &mut SqliteConnection, shift: Uuid) -> Result<Uuid, AppError> {
    let id: String = sqlx::query_scalar("SELECT location_id FROM shifts WHERE id=?")
        .bind(shift.to_string())
        .fetch_optional(c)
        .await?
        .ok_or_else(|| AppError::NotFound("Shift not found".into()))?;
    Uuid::parse_str(&id).map_err(|e| AppError::Internal(e.to_string()))
}

impl TenantShiftRepository {
    async fn get(c: &mut SqliteConnection, id: Uuid) -> Result<Shift, AppError> {
        sqlx::query_as(&format!("{} WHERE id=?", Self::SELECT))
            .bind(id.to_string())
            .fetch_optional(c)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Shift {id} not found")))
    }
    async fn staff(c: &mut SqliteConnection, user: Uuid) -> Result<(), AppError> {
        let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id=? AND role IN ('staff','admin') AND deleted_at IS NULL AND is_active=1)").bind(user.to_string()).fetch_one(c).await?;
        if !valid {
            return Err(AppError::Forbidden(
                "Only active staff can open shifts".into(),
            ));
        }
        Ok(())
    }
    async fn create_on(
        c: &mut SqliteConnection,
        user: Uuid,
        venue: Uuid,
        notes: Option<String>,
        actor: Uuid,
    ) -> Result<Shift, AppError> {
        Self::staff(c, user).await?;
        let playing: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM usage_sessions WHERE player_id=? AND end_time IS NULL AND deleted_at IS NULL)").bind(user.to_string()).fetch_one(&mut *c).await?;
        if playing {return Err(AppError::conflict_code("STAFF_GAMING_SESSION_ACTIVE",None));}
        let id = Uuid::now_v7();
        let ts = now()?;
        sqlx::query("INSERT INTO shifts(id,user_id,location_id,clock_in,notes,created_by,updated_by,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)").bind(id.to_string()).bind(user.to_string()).bind(venue.to_string()).bind(&ts).bind(notes).bind(actor.to_string()).bind(actor.to_string()).bind(&ts).bind(&ts).execute(&mut *c).await.map_err(|e| match &e {
            sqlx::Error::Database(d) if d.is_unique_violation() => AppError::Conflict("User already has an active shift".into()),
            _ => AppError::Database(e),
        })?;
        let row = Self::get(c, id).await?;
        event(
            c,
            "shift",
            id,
            "shift.started",
            Some(venue),
            false,
            json!(row),
        )
        .await?;
        Ok(row)
    }
    pub async fn create(
        &self,
        user: Uuid,
        venue: Uuid,
        notes: Option<String>,
        actor: Uuid,
    ) -> Result<Shift, AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move { Self::create_on(c, user, venue, notes, actor).await })
            }),
        )
        .await
    }
    pub async fn start_confirmed(
        &self,
        user: Uuid,
        dto: StartShiftDto,
        actor: Uuid,
    ) -> Result<ShiftStartResponseDto, AppError> {
        let venue = dto
            .venue_location_id
            .ok_or_else(|| AppError::bad_request_code("LOCATION_REQUIRED", None))?;
        nonnegative(dto.opening_balance)?;
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    Self::staff(c, user).await?;
                    let active: Option<Shift> = sqlx::query_as(&format!(
                        "{} WHERE user_id=? AND status='active'",
                        Self::SELECT
                    ))
                    .bind(user.to_string())
                    .fetch_optional(&mut *c)
                    .await?;
                    if let Some(shift) = active {
                        if shift_location(c, shift.id).await? != venue {
                            return Err(AppError::Conflict(
                                "Finish the active shift at its current location".into(),
                            ));
                        }
                        if let Some(register) = TenantCashRegisterRepository::by_shift(c, shift.id)
                            .await?
                            .filter(|x| x.status == "open")
                        {
                            return Ok(ShiftStartResponseDto {
                                resumed: true,
                                shift,
                                cash_register: register,
                            });
                        }
                        Self::close_on(
                            c,
                            shift.id,
                            Some("Recovered stale shift".into()),
                            actor,
                            false,
                        )
                        .await?;
                    }
                    let shift = Self::create_on(c, user, venue, dto.notes.clone(), actor).await?;
                    let cash_register = TenantCashRegisterRepository::open_on(
                        c,
                        &OpenCashRegisterDto {
                            shift_id: shift.id,
                            opening_balance: dto.opening_balance,
                            opening_denominations: dto.opening_denominations,
                            notes: dto.notes,
                        },
                        actor,
                    )
                    .await?;
                    Ok(ShiftStartResponseDto {
                        resumed: false,
                        shift,
                        cash_register,
                    })
                })
            }),
        )
        .await
    }
    async fn close_on(
        c: &mut SqliteConnection,
        id: Uuid,
        notes: Option<String>,
        actor: Uuid,
        force: bool,
    ) -> Result<Shift, AppError> {
        let old = Self::get(c, id).await?;
        if old.status != "active" {
            return Err(AppError::Conflict("Shift is already closed".into()));
        }
        let venue = shift_location(c, id).await?;
        if let Some(register) = TenantCashRegisterRepository::by_shift(c, id)
            .await?
            .filter(|x| x.status == "open")
        {
            let expected = TenantCashRegisterRepository::ledger_expected(c, register.id).await?;
            TenantCashRegisterRepository::close_scaled(
                c,
                register.id,
                expected,
                None,
                Some("Auto-closed on shift end".into()),
                actor,
            )
            .await?;
        }
        let ts = now()?;
        sqlx::query("UPDATE shifts SET status='closed',close_kind=?,clock_out=?,notes=COALESCE(?,notes),updated_by=?,updated_at=? WHERE id=?").bind(if force { "force" } else { "normal" }).bind(&ts).bind(notes).bind(actor.to_string()).bind(&ts).bind(id.to_string()).execute(&mut *c).await?;
        let row = Self::get(c, id).await?;
        event(
            c,
            "shift",
            id,
            "shift.ended",
            Some(venue),
            false,
            json!(row),
        )
        .await?;
        Ok(row)
    }
    pub async fn close(
        &self,
        id: Uuid,
        notes: Option<String>,
        actor: Uuid,
    ) -> Result<Shift, AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move { Self::close_on(c, id, notes, actor, false).await })
            }),
        )
        .await
    }
    pub async fn force_close(&self, id: Uuid, actor: Uuid) -> Result<Shift, AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move { Self::close_on(c, id, None, actor, true).await })
            }),
        )
        .await
    }
    pub async fn close_with_register(
        &self,
        id: Uuid,
        dto: ShiftCloseDto,
        actor: Uuid,
    ) -> Result<ShiftCloseResponseDto, AppError> {
        write(
            &self.db,
            Box::new(move |c| Box::pin(async move { Self::close_bundle(c, id, dto, actor).await })),
        )
        .await
    }
    async fn close_bundle(
        c: &mut SqliteConnection,
        id: Uuid,
        dto: ShiftCloseDto,
        actor: Uuid,
    ) -> Result<ShiftCloseResponseDto, AppError> {
        let shift = Self::get(c, id).await?;
        if shift.status != "active" {
            return Err(AppError::Conflict("Shift is already closed".into()));
        }
        let register = TenantCashRegisterRepository::by_shift(c, id).await?;
        let mut deposit = None;
        let mut closed = None;
        if let Some(register) = register {
            if register.status == "open" {
                if let Some(input) = dto.deposit {
                    deposit = Some(
                        TenantCashDepositRepository::create_on(
                            c,
                            &InitiateDepositDto {
                                cash_register_id: register.id,
                                shift_id: id,
                                amount: input.amount,
                                denominations: input.denominations,
                                notes: input.notes,
                            },
                            shift.user_id,
                        )
                        .await?,
                    );
                }
                closed = Some(
                    TenantCashRegisterRepository::close_on(
                        c,
                        register.id,
                        &CloseCashRegisterDto {
                            closing_balance: dto.closing_balance,
                            closing_denominations: dto.closing_denominations,
                            notes: dto.notes.clone(),
                        },
                        actor,
                    )
                    .await?,
                );
            } else {
                let stored: i64 =
                    sqlx::query_scalar("SELECT closing_balance FROM cash_registers WHERE id=?")
                        .bind(register.id.to_string())
                        .fetch_one(&mut *c)
                        .await?;
                if stored != nonnegative(dto.closing_balance)? || dto.deposit.is_some() {
                    return Err(AppError::Conflict(
                        "Cash register is already closed with different input".into(),
                    ));
                }
                closed = Some(register);
            }
        } else if dto.deposit.is_some() {
            return Err(AppError::Conflict(
                "Cannot deposit without a register".into(),
            ));
        }
        let closed_shift = Self::close_on(c, id, dto.notes, actor, false).await?;
        Ok(ShiftCloseResponseDto {
            closedShift: closed_shift,
            cashRegister: closed,
            deposit,
        })
    }
    /// Call only after authenticating and authorizing the incoming staff identity in the control plane.
    pub async fn handover(
        &self,
        id: Uuid,
        incoming: Uuid,
        dto: ShiftCloseDto,
        actor: Uuid,
    ) -> Result<(ShiftCloseResponseDto, Shift, CashRegister), AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let current = Self::get(c, id).await?;
                    if current.user_id == incoming {
                        return Err(AppError::BadRequest("Handover requires different staff".into()));
                    }
                    let venue = shift_location(c, id).await?;
                    Self::staff(c, incoming).await?;
                    let closed = Self::close_bundle(c, id, dto, actor).await?;
                    let opening = if let Some(register) = &closed.cashRegister { TenantCashRegisterRepository::carry_forward_on(c, register.id).await? } else { 0 };
                    let shift = Self::create_on(c, incoming, venue, Some("Auto-started on handover".into()), incoming).await?;
                    let register = TenantCashRegisterRepository::open_scaled(c, shift.id, opening, None, Some("Handover float".into()), incoming).await?;
                    event(c, "shift", id, "shift.handover", Some(venue), false, json!({"closedShiftId":id,"newShiftId":shift.id,"fromStaffId":current.user_id,"toStaffId":incoming})).await?;
                    Ok((closed, shift, register))
                })
            }),
        )
        .await
    }
}

impl TenantCashRegisterRepository {
    pub(crate) async fn get(c: &mut SqliteConnection, id: Uuid) -> Result<CashRegister, AppError> {
        sqlx::query_as(&format!("{} WHERE id=?", Self::SELECT))
            .bind(id.to_string())
            .fetch_optional(c)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Cash register {id} not found")))
    }
    async fn by_shift(
        c: &mut SqliteConnection,
        id: Uuid,
    ) -> Result<Option<CashRegister>, AppError> {
        Ok(sqlx::query_as(&format!(
            "{} WHERE shift_id=? ORDER BY (status='open') DESC,created_at DESC,id DESC LIMIT 1",
            Self::SELECT
        ))
        .bind(id.to_string())
        .fetch_optional(c)
        .await?)
    }
    async fn open_scaled(
        c: &mut SqliteConnection,
        shift: Uuid,
        opening: i64,
        denominations: Option<Value>,
        notes: Option<String>,
        actor: Uuid,
    ) -> Result<CashRegister, AppError> {
        if opening < 0 {
            return Err(AppError::BadRequest(
                "Opening balance must be nonnegative".into(),
            ));
        }
        let state: Option<String> = sqlx::query_scalar("SELECT status FROM shifts WHERE id=?")
            .bind(shift.to_string())
            .fetch_optional(&mut *c)
            .await?;
        if state.as_deref() != Some("active") {
            return Err(AppError::Conflict(
                "Only active shifts can open registers".into(),
            ));
        }
        let id = Uuid::now_v7();
        let ts = now()?;
        sqlx::query("INSERT INTO cash_registers(id,shift_id,opened_by,opening_balance,opening_denominations,notes,created_by,updated_by,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?)").bind(id.to_string()).bind(shift.to_string()).bind(actor.to_string()).bind(opening).bind(json_option(denominations)?).bind(notes).bind(actor.to_string()).bind(actor.to_string()).bind(&ts).bind(&ts).execute(&mut *c).await.map_err(|e| match &e {
            sqlx::Error::Database(d) if d.is_unique_violation() => AppError::Conflict("An open register already exists for this shift".into()),
            _ => AppError::Database(e),
        })?;
        let row = Self::get(c, id).await?;
        let venue = shift_location(c, shift).await?;
        event(
            c,
            "cash_register",
            id,
            "cash_register.opened",
            Some(venue),
            false,
            json!(row),
        )
        .await?;
        Ok(row)
    }
    async fn open_on(
        c: &mut SqliteConnection,
        dto: &OpenCashRegisterDto,
        actor: Uuid,
    ) -> Result<CashRegister, AppError> {
        Self::open_scaled(
            c,
            dto.shift_id,
            nonnegative(dto.opening_balance)?,
            dto.opening_denominations.clone(),
            dto.notes.clone(),
            actor,
        )
        .await
    }
    pub async fn open_register(
        &self,
        dto: &OpenCashRegisterDto,
        actor: Uuid,
    ) -> Result<CashRegister, AppError> {
        let dto = dto.clone();
        write(
            &self.db,
            Box::new(move |c| Box::pin(async move { Self::open_on(c, &dto, actor).await })),
        )
        .await
    }
    pub(crate) async fn ledger_expected(
        c: &mut SqliteConnection,
        id: Uuid,
    ) -> Result<i64, AppError> {
        let opening: i64 =
            sqlx::query_scalar("SELECT opening_balance FROM cash_registers WHERE id=?")
                .bind(id.to_string())
                .fetch_one(&mut *c)
                .await?;
        // Deposit reservation and reversal entries are excluded together; rejection cannot create cash.
        let (incoming, outgoing): (i64, i64) = sqlx::query_as("SELECT COALESCE(SUM(CASE WHEN entry_type='cash_in' THEN amount ELSE 0 END),0),COALESCE(SUM(CASE WHEN entry_type='cash_out' THEN amount ELSE 0 END),0) FROM cash_register_entries WHERE cash_register_id=? AND COALESCE(reference_type,'')<>'cash_deposit'").bind(id.to_string()).fetch_one(c).await?;
        checked_sub(checked_add(opening, incoming)?, outgoing)
    }
    async fn approved_deposits(c: &mut SqliteConnection, id: Uuid) -> Result<i64, AppError> {
        Ok(sqlx::query_scalar("SELECT COALESCE(SUM(amount),0) FROM cash_deposits WHERE cash_register_id=? AND status='approved'").bind(id.to_string()).fetch_one(c).await?)
    }
    async fn close_scaled(
        c: &mut SqliteConnection,
        id: Uuid,
        closing: i64,
        denominations: Option<Value>,
        notes: Option<String>,
        actor: Uuid,
    ) -> Result<CashRegister, AppError> {
        let old = Self::get(c, id).await?;
        if old.status != "open" {
            return Err(AppError::Conflict("Cash register is not open".into()));
        }
        let expected = Self::ledger_expected(c, id).await?;
        if expected < 0 {
            return Err(AppError::Conflict(
                "Cash ledger cannot have a negative expected balance".into(),
            ));
        }
        let approved = Self::approved_deposits(c, id).await?;
        let variance = checked_sub(checked_sub(closing, expected)?, approved)?;
        let ts = now()?;
        sqlx::query("UPDATE cash_registers SET closing_balance=?,closing_denominations=?,expected_closing=?,variance=?,status='closed',closed_by=?,notes=COALESCE(?,notes),updated_by=?,updated_at=? WHERE id=?").bind(closing).bind(json_option(denominations)?).bind(expected).bind(variance).bind(actor.to_string()).bind(notes).bind(actor.to_string()).bind(&ts).bind(id.to_string()).execute(&mut *c).await?;
        let row = Self::get(c, id).await?;
        let venue = shift_location(c, row.shift_id).await?;
        event(
            c,
            "cash_register",
            id,
            "cash_register.closed",
            Some(venue),
            false,
            json!(row),
        )
        .await?;
        Ok(row)
    }
    async fn close_on(
        c: &mut SqliteConnection,
        id: Uuid,
        dto: &CloseCashRegisterDto,
        actor: Uuid,
    ) -> Result<CashRegister, AppError> {
        Self::close_scaled(
            c,
            id,
            nonnegative(dto.closing_balance)?,
            dto.closing_denominations.clone(),
            dto.notes.clone(),
            actor,
        )
        .await
    }
    pub async fn close_register(
        &self,
        id: Uuid,
        dto: &CloseCashRegisterDto,
        actor: Uuid,
    ) -> Result<CashRegister, AppError> {
        let dto = dto.clone();
        write(
            &self.db,
            Box::new(move |c| Box::pin(async move { Self::close_on(c, id, &dto, actor).await })),
        )
        .await
    }
    pub async fn reconcile(
        &self,
        id: Uuid,
        notes: Option<String>,
        actor: Uuid,
    ) -> Result<CashRegister, AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let old = Self::get(c, id).await?;
                    if old.status != "closed" {
                        return Err(AppError::Conflict("Only closed registers can be reconciled".into()));
                    }
                    let ts = now()?;
                    sqlx::query("UPDATE cash_registers SET status='reconciled',reconciled_by=?,reconciled_at=?,reconciliation_notes=?,updated_by=?,updated_at=? WHERE id=?").bind(actor.to_string()).bind(&ts).bind(notes).bind(actor.to_string()).bind(&ts).bind(id.to_string()).execute(&mut *c).await?;
                    let row = Self::get(c, id).await?;
                    let venue = shift_location(c, row.shift_id).await?;
                    event(c, "cash_register", id, "cash_register.reconciled", Some(venue), false, json!(row)).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn update_opening_balance(
        &self,
        id: Uuid,
        amount: f64,
        denominations: Option<Value>,
        actor: Uuid,
    ) -> Result<CashRegister, AppError> {
        let amount = nonnegative(amount)?;
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let old = Self::get(c, id).await?;
                    if old.status != "open" {
                        return Err(AppError::Conflict("Only open registers can change their opening balance".into()));
                    }
                    sqlx::query("UPDATE cash_registers SET opening_balance=?,opening_denominations=?,updated_by=?,updated_at=? WHERE id=?").bind(amount).bind(json_option(denominations)?).bind(actor.to_string()).bind(now()?).bind(id.to_string()).execute(&mut *c).await?;
                    let row = Self::get(c, id).await?;
                    let venue = shift_location(c, row.shift_id).await?;
                    event(c, "cash_register", id, "cash_register.updated", Some(venue), false, json!(row)).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub(crate) async fn entry_on(
        c: &mut SqliteConnection,
        id: Uuid,
        kind: &str,
        amount: i64,
        reason: Option<String>,
        reference: Option<Uuid>,
        reference_type: Option<String>,
        actor: Uuid,
        allow_closed: bool,
    ) -> Result<CashRegisterEntry, AppError> {
        let old = Self::get(c, id).await?;
        if old.status != "open" && !allow_closed {
            return Err(AppError::Conflict(
                "Cannot add entries to a closed register".into(),
            ));
        }
        if amount <= 0
            || !matches!(kind, "cash_in" | "cash_out")
            || reference.is_some() != reference_type.is_some()
        {
            return Err(AppError::BadRequest(
                "Invalid cash entry amount, direction or reference".into(),
            ));
        }
        let entry = Uuid::now_v7();
        let ts = now()?;
        sqlx::query("INSERT INTO cash_register_entries(id,cash_register_id,entry_type,amount,reason,reference_id,reference_type,created_by,created_at) VALUES(?,?,?,?,?,?,?,?,?)").bind(entry.to_string()).bind(id.to_string()).bind(kind).bind(amount).bind(reason).bind(reference.map(|x| x.to_string())).bind(reference_type).bind(actor.to_string()).bind(ts).execute(&mut *c).await?;
        let row: CashRegisterEntry = sqlx::query_as(&format!("{} WHERE id=?", Self::ENTRY_SELECT))
            .bind(entry.to_string())
            .fetch_one(&mut *c)
            .await?;
        let venue = shift_location(c, old.shift_id).await?;
        event(
            c,
            "cash_register_entry",
            entry,
            "cash_register.entry_created",
            Some(venue),
            false,
            json!(row),
        )
        .await?;
        Ok(row)
    }
    pub(crate) async fn record_tender_on(
        c: &mut SqliteConnection,
        shift: Option<Uuid>,
        actor: Option<Uuid>,
        cash: i64,
        reference: Uuid,
        reference_type: &str,
    ) -> Result<(), AppError> {
        if cash <= 0 {
            return Ok(());
        }
        let (Some(shift), Some(actor)) = (shift, actor) else {
            return Ok(());
        };
        let register = Self::by_shift(c, shift)
            .await?
            .filter(|r| r.status == "open")
            .ok_or_else(|| AppError::Conflict("Cash payment requires an open register".into()))?;
        Self::entry_on(
            c,
            register.id,
            "cash_in",
            cash,
            None,
            Some(reference),
            Some(reference_type.into()),
            actor,
            false,
        )
        .await?;
        Ok(())
    }
    pub async fn add_entry(
        &self,
        id: Uuid,
        dto: &CreateCashRegisterEntryDto,
        actor: Uuid,
    ) -> Result<CashRegisterEntry, AppError> {
        let dto = dto.clone();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    Self::entry_on(
                        c,
                        id,
                        &dto.entry_type,
                        positive(dto.amount)?,
                        dto.reason,
                        dto.reference_id,
                        dto.reference_type,
                        actor,
                        false,
                    )
                    .await
                })
            }),
        )
        .await
    }
    pub async fn get_expected_closing(&self, id: Uuid) -> Result<f64, AppError> {
        let mut tx = self.db.read_pool()?.begin().await?;
        money_f64(Self::ledger_expected(&mut tx, id).await?)
    }
    async fn recalculate_on(
        c: &mut SqliteConnection,
        id: Uuid,
    ) -> Result<Option<CashRegister>, AppError> {
        let old = Self::get(c, id).await?;
        if !matches!(old.status.as_str(), "closed" | "reconciled") {
            return Ok(None);
        }
        let closing: i64 =
            sqlx::query_scalar("SELECT closing_balance FROM cash_registers WHERE id=?")
                .bind(id.to_string())
                .fetch_one(&mut *c)
                .await?;
        let expected = Self::ledger_expected(c, id).await?;
        let approved = Self::approved_deposits(c, id).await?;
        let variance = checked_sub(checked_sub(closing, expected)?, approved)?;
        sqlx::query(
            "UPDATE cash_registers SET expected_closing=?,variance=?,updated_at=? WHERE id=?",
        )
        .bind(expected)
        .bind(variance)
        .bind(now()?)
        .bind(id.to_string())
        .execute(&mut *c)
        .await?;
        let row = Self::get(c, id).await?;
        let venue = shift_location(c, row.shift_id).await?;
        event(
            c,
            "cash_register",
            id,
            "cash_register.updated",
            Some(venue),
            false,
            json!(row),
        )
        .await?;
        Ok(Some(row))
    }
    pub async fn recalculate_closed_register_totals(
        &self,
        id: Uuid,
    ) -> Result<Option<CashRegister>, AppError> {
        write(
            &self.db,
            Box::new(move |c| Box::pin(async move { Self::recalculate_on(c, id).await })),
        )
        .await
    }
    async fn carry_forward_on(c: &mut SqliteConnection, id: Uuid) -> Result<i64, AppError> {
        let closing: Option<i64> = sqlx::query_scalar("SELECT closing_balance FROM cash_registers WHERE id=? AND status IN ('closed','reconciled')").bind(id.to_string()).fetch_optional(&mut *c).await?.ok_or_else(|| AppError::Conflict("Carry forward requires a closed register".into()))?;
        let reserved: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(amount),0) FROM cash_deposits WHERE cash_register_id=? AND status IN ('pending','approved')").bind(id.to_string()).fetch_one(c).await?;
        checked_sub(closing.unwrap_or(0), reserved)
    }
    pub async fn preview_carry_forward_balance_for(&self, venue: Uuid) -> Result<f64, AppError> {
        let mut tx = self.db.read_pool()?.begin().await?;
        let id: Option<String> = sqlx::query_scalar("SELECT cr.id FROM cash_registers cr JOIN shifts s ON s.id=cr.shift_id WHERE s.location_id=? AND cr.status IN ('closed','reconciled') ORDER BY cr.updated_at DESC,cr.id DESC LIMIT 1").bind(venue.to_string()).fetch_optional(&mut *tx).await?;
        match id {
            Some(id) => money_f64(
                Self::carry_forward_on(
                    &mut tx,
                    Uuid::parse_str(&id).map_err(|e| AppError::Internal(e.to_string()))?,
                )
                .await?,
            ),
            None => Ok(0.0),
        }
    }
    pub async fn get_by_id(&self, id: Uuid) -> Result<CashRegisterWithEntries, AppError> {
        let register = self
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound("Cash register not found".into()))?;
        Ok(CashRegisterWithEntries {
            register,
            entries: self.list_entries(id).await?,
        })
    }
    pub async fn get_by_shift(&self, id: Uuid) -> Result<CashRegisterWithEntries, AppError> {
        let register = self
            .find_by_shift(id)
            .await?
            .ok_or_else(|| AppError::NotFound("Cash register not found".into()))?;
        Ok(CashRegisterWithEntries {
            entries: self.list_entries(register.id).await?,
            register,
        })
    }
}

impl TenantCashDepositRepository {
    async fn get(c: &mut SqliteConnection, id: Uuid) -> Result<CashDeposit, AppError> {
        sqlx::query_as(&format!("{} WHERE id=?", Self::SELECT))
            .bind(id.to_string())
            .fetch_optional(c)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Deposit {id} not found")))
    }
    async fn create_on(
        c: &mut SqliteConnection,
        dto: &InitiateDepositDto,
        actor: Uuid,
    ) -> Result<CashDeposit, AppError> {
        let amount = positive(dto.amount)?;
        if !dto.denominations.is_object() {
            return Err(AppError::BadRequest(
                "Deposit denominations must be an object".into(),
            ));
        }
        let register = TenantCashRegisterRepository::get(c, dto.cash_register_id).await?;
        if register.status != "open" || register.shift_id != dto.shift_id {
            return Err(AppError::Conflict(
                "Deposit requires its shift's open register".into(),
            ));
        }
        let user: String =
            sqlx::query_scalar("SELECT user_id FROM shifts WHERE id=? AND status='active'")
                .bind(dto.shift_id.to_string())
                .fetch_optional(&mut *c)
                .await?
                .ok_or_else(|| AppError::Conflict("Deposit requires an active shift".into()))?;
        if user != actor.to_string() {
            return Err(AppError::Forbidden(
                "Only the shift owner can initiate its deposit".into(),
            ));
        }
        let id = Uuid::now_v7();
        let ts = now()?;
        sqlx::query("INSERT INTO cash_deposits(id,cash_register_id,shift_id,initiated_by,amount,denominations,notes,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)").bind(id.to_string()).bind(dto.cash_register_id.to_string()).bind(dto.shift_id.to_string()).bind(actor.to_string()).bind(amount).bind(serde_json::to_string(&dto.denominations).map_err(|e| AppError::BadRequest(e.to_string()))?).bind(&dto.notes).bind(&ts).bind(&ts).execute(&mut *c).await?;
        TenantCashRegisterRepository::entry_on(
            c,
            dto.cash_register_id,
            "cash_out",
            amount,
            Some(format!("Cash deposit pending approval ({id})")),
            Some(id),
            Some("cash_deposit".into()),
            actor,
            false,
        )
        .await?;
        let row = Self::get(c, id).await?;
        let venue = shift_location(c, row.shift_id).await?;
        event(
            c,
            "cash_deposit",
            id,
            "approval.requested",
            Some(venue),
            false,
            json!({"deposit_id":id,"entity_type":"cash_deposit","requestedBy":actor,"deposit":row}),
        )
        .await?;
        Ok(row)
    }
    pub async fn create(
        &self,
        dto: &InitiateDepositDto,
        actor: Uuid,
    ) -> Result<CashDeposit, AppError> {
        let dto = dto.clone();
        write(
            &self.db,
            Box::new(move |c| Box::pin(async move { Self::create_on(c, &dto, actor).await })),
        )
        .await
    }
    pub async fn approve(
        &self,
        id: Uuid,
        kind: &str,
        actor: Uuid,
    ) -> Result<CashDeposit, AppError> {
        if !matches!(kind, "bank" | "home") {
            return Err(AppError::BadRequest(
                "Deposit type must be bank or home".into(),
            ));
        }
        self.decide(id, Some(kind.to_string()), None, actor).await
    }
    pub async fn reject(
        &self,
        id: Uuid,
        reason: &str,
        actor: Uuid,
    ) -> Result<CashDeposit, AppError> {
        if reason.trim().is_empty() {
            return Err(AppError::BadRequest("Rejection reason is required".into()));
        }
        self.decide(id, None, Some(reason.trim().into()), actor)
            .await
    }
    async fn decide(
        &self,
        id: Uuid,
        kind: Option<String>,
        reason: Option<String>,
        actor: Uuid,
    ) -> Result<CashDeposit, AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let old = Self::get(c, id).await?;
                    if old.status != "pending" {
                        return Err(AppError::Conflict("Deposit is not pending".into()));
                    }
                    let status = if reason.is_some() { "rejected" } else { "approved" };
                    let ts = now()?;
                    sqlx::query("UPDATE cash_deposits SET status=?,deposit_type=?,rejection_reason=?,approved_by=?,approved_at=?,updated_at=? WHERE id=?").bind(status).bind(kind).bind(reason).bind(actor.to_string()).bind(&ts).bind(&ts).bind(id.to_string()).execute(&mut *c).await?;
                    if status == "rejected" {
                        let amount: i64 = sqlx::query_scalar("SELECT amount FROM cash_deposits WHERE id=?").bind(id.to_string()).fetch_one(&mut *c).await?;
                        TenantCashRegisterRepository::entry_on(c, old.cash_register_id, "cash_in", amount, Some(format!("Deposit {id} rejected: reversal")), Some(id), Some("cash_deposit".into()), actor, true).await?;
                    }
                    TenantCashRegisterRepository::recalculate_on(c, old.cash_register_id).await?;
                    let row = Self::get(c, id).await?;
                    let venue = shift_location(c, row.shift_id).await?;
                    event(c, "cash_deposit", id, "approval.decided", Some(venue), false, json!({"deposit_id":id,"entity_type":"cash_deposit","status":status,"requestedBy":row.initiated_by,"deposit":row})).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
}

impl TenantExpenseCategoryRepository {
    async fn get(c: &mut SqliteConnection, id: Uuid) -> Result<ExpenseCategory, AppError> {
        sqlx::query_as(&format!("{} WHERE id=?", Self::SELECT))
            .bind(id.to_string())
            .fetch_optional(c)
            .await?
            .ok_or_else(|| AppError::NotFound("Expense category not found".into()))
    }
    async fn validate_parent(
        c: &mut SqliteConnection,
        id: Uuid,
        parent: Option<Uuid>,
    ) -> Result<(), AppError> {
        if let Some(parent) = parent {
            Self::get(c, parent).await?;
            let cycle: bool = sqlx::query_scalar("WITH RECURSIVE ancestors(id,parent_id) AS (SELECT id,parent_id FROM expense_categories WHERE id=? UNION SELECT e.id,e.parent_id FROM expense_categories e JOIN ancestors a ON a.parent_id=e.id) SELECT EXISTS(SELECT 1 FROM ancestors WHERE id=?)").bind(parent.to_string()).bind(id.to_string()).fetch_one(c).await?;
            if cycle {
                return Err(AppError::BadRequest(
                    "Expense category hierarchy cannot contain a cycle".into(),
                ));
            }
        }
        Ok(())
    }
    async fn unique(
        c: &mut SqliteConnection,
        name: &str,
        id: Option<Uuid>,
    ) -> Result<(), AppError> {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM expense_categories WHERE lower(name)=lower(?) AND (? IS NULL OR id<>?))").bind(name).bind(id.map(|x| x.to_string())).bind(id.map(|x| x.to_string())).fetch_one(c).await?;
        if exists {
            return Err(AppError::Conflict(format!(
                "Expense category '{name}' already exists"
            )));
        }
        Ok(())
    }
    pub async fn create(
        &self,
        dto: &CreateExpenseCategoryDto,
        actor: Option<Uuid>,
    ) -> Result<ExpenseCategory, AppError> {
        let dto = dto.clone();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let id = Uuid::now_v7();
                    Self::unique(c, &dto.name, None).await?;
                    Self::validate_parent(c, id, dto.parent_id).await?;
                    let ts = now()?;
                    sqlx::query("INSERT INTO expense_categories(id,name,description,parent_id,is_active,budget_amount,budget_period,created_by,updated_by,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)").bind(id.to_string()).bind(dto.name).bind(dto.description).bind(dto.parent_id.map(|x| x.to_string())).bind(dto.is_active.unwrap_or(true)).bind(dto.budget_amount.map(nonnegative).transpose()?).bind(dto.budget_period).bind(actor.map(|x| x.to_string())).bind(actor.map(|x| x.to_string())).bind(&ts).bind(&ts).execute(&mut *c).await?;
                    let row = Self::get(c, id).await?;
                    event(c, "expense_category", id, "expense_category.created", None, false, json!(row)).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdateExpenseCategoryDto,
        actor: Option<Uuid>,
    ) -> Result<ExpenseCategory, AppError> {
        let dto = dto.clone();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    Self::get(c, id).await?;
                    if let Some(name) = &dto.name {
                        Self::unique(c, name, Some(id)).await?;
                    }
                    Self::validate_parent(c, id, dto.parent_id).await?;
                    sqlx::query("UPDATE expense_categories SET name=COALESCE(?,name),description=COALESCE(?,description),parent_id=COALESCE(?,parent_id),is_active=COALESCE(?,is_active),budget_amount=COALESCE(?,budget_amount),budget_period=COALESCE(?,budget_period),updated_by=?,updated_at=? WHERE id=?").bind(dto.name).bind(dto.description).bind(dto.parent_id.map(|x| x.to_string())).bind(dto.is_active).bind(dto.budget_amount.map(nonnegative).transpose()?).bind(dto.budget_period).bind(actor.map(|x| x.to_string())).bind(now()?).bind(id.to_string()).execute(&mut *c).await?;
                    let row = Self::get(c, id).await?;
                    event(c, "expense_category", id, "expense_category.updated", None, false, json!(row)).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    Self::get(c, id).await?;
                    sqlx::query("DELETE FROM expense_categories WHERE id=?")
                        .bind(id.to_string())
                        .execute(&mut *c)
                        .await?;
                    event(
                        c,
                        "expense_category",
                        id,
                        "expense_category.deleted",
                        None,
                        true,
                        json!({"id":id}),
                    )
                    .await
                })
            }),
        )
        .await
    }
}

impl TenantExpenseRepository {
    async fn get(
        c: &mut SqliteConnection,
        id: Uuid,
        include_deleted: bool,
    ) -> Result<Expense, AppError> {
        sqlx::query_as(&format!(
            "{} WHERE id=? {}",
            Self::SELECT,
            if include_deleted {
                ""
            } else {
                "AND deleted_at IS NULL"
            }
        ))
        .bind(id.to_string())
        .fetch_optional(c)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Expense {id} not found")))
    }
    async fn references(
        c: &mut SqliteConnection,
        category: Uuid,
        vendor: Option<Uuid>,
        shift: Option<Uuid>,
    ) -> Result<(), AppError> {
        TenantExpenseCategoryRepository::get(c, category).await?;
        if let Some(vendor) = vendor {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM vendors WHERE id=?)")
                    .bind(vendor.to_string())
                    .fetch_one(&mut *c)
                    .await?;
            if !exists {
                return Err(AppError::BadRequest("Expense vendor not found".into()));
            }
        }
        if let Some(shift) = shift {
            TenantShiftRepository::get(c, shift).await?;
        }
        Ok(())
    }
    async fn scope(
        c: &mut SqliteConnection,
        shift: Option<Uuid>,
    ) -> Result<Option<Uuid>, AppError> {
        match shift {
            Some(id) => Ok(Some(shift_location(c, id).await?)),
            None => Ok(None),
        }
    }
    pub async fn create(
        &self,
        dto: &CreateExpenseDto,
        actor: Option<Uuid>,
    ) -> Result<Expense, AppError> {
        let dto = dto.clone();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let amount = positive(dto.amount)?;
                    Self::references(c, dto.category_id, dto.vendor_id, dto.shift_id).await?;
                    let id = Uuid::now_v7();
                    let ts = now()?;
                    let venue = Self::scope(c, dto.shift_id).await?;
                    sqlx::query("INSERT INTO expenses(id,category_id,vendor_id,amount,payment_method,payment_account,description,receipt_url,expense_date,is_recurring,recurrence_pattern,shift_id,created_by,updated_by,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)").bind(id.to_string()).bind(dto.category_id.to_string()).bind(dto.vendor_id.map(|x| x.to_string())).bind(amount).bind(dto.payment_method).bind(dto.payment_account).bind(dto.description).bind(dto.receipt_url).bind(date(dto.expense_date)?.unwrap_or_else(|| ts.clone())).bind(dto.is_recurring.unwrap_or(false)).bind(dto.recurrence_pattern).bind(dto.shift_id.map(|x| x.to_string())).bind(actor.map(|x| x.to_string())).bind(actor.map(|x| x.to_string())).bind(&ts).bind(&ts).execute(&mut *c).await?;
                    let row = Self::get(c, id, false).await?;
                    event(c, "expense", id, "expense.created", venue, false, json!(row)).await?;
                    event(c, "expense", id, "approval.requested", venue, false, json!({"expense_id":id,"entity_type":"expense","amount":row.amount,"requestedBy":actor})).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdateExpenseDto,
        actor: Option<Uuid>,
    ) -> Result<Expense, AppError> {
        let dto = dto.clone();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let old = Self::get(c, id, false).await?;
                    if old.approval_status == "approved" {
                        return Err(AppError::Conflict("Cannot update an approved expense".into()));
                    }
                    let category = dto.category_id.unwrap_or(old.category_id);
                    let vendor = dto.vendor_id.or(old.vendor_id);
                    let shift = dto.shift_id.or(old.shift_id);
                    Self::references(c, category, vendor, shift).await?;
                    sqlx::query("UPDATE expenses SET category_id=?,vendor_id=?,amount=COALESCE(?,amount),payment_method=COALESCE(?,payment_method),payment_account=COALESCE(?,payment_account),description=COALESCE(?,description),receipt_url=COALESCE(?,receipt_url),expense_date=COALESCE(?,expense_date),is_recurring=COALESCE(?,is_recurring),recurrence_pattern=COALESCE(?,recurrence_pattern),shift_id=?,updated_by=?,updated_at=? WHERE id=?").bind(category.to_string()).bind(vendor.map(|x| x.to_string())).bind(dto.amount.map(positive).transpose()?).bind(dto.payment_method).bind(dto.payment_account).bind(dto.description).bind(dto.receipt_url).bind(date(dto.expense_date)?).bind(dto.is_recurring).bind(dto.recurrence_pattern).bind(shift.map(|x| x.to_string())).bind(actor.map(|x| x.to_string())).bind(now()?).bind(id.to_string()).execute(&mut *c).await?;
                    let row = Self::get(c, id, false).await?;
                    let venue = Self::scope(c, row.shift_id).await?;
                    event(c, "expense", id, "expense.updated", venue, false, json!(row)).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn soft_delete(&self, id: Uuid) -> Result<Expense, AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let old = Self::get(c, id, false).await?;
                    let ts = now()?;
                    sqlx::query("UPDATE expenses SET deleted_at=?,updated_at=? WHERE id=?")
                        .bind(&ts)
                        .bind(&ts)
                        .bind(id.to_string())
                        .execute(&mut *c)
                        .await?;
                    let row = Self::get(c, id, true).await?;
                    let venue = Self::scope(c, old.shift_id).await?;
                    event(c, "expense", id, "expense.deleted", venue, true, json!(row)).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn approve(&self, id: Uuid, actor: Uuid) -> Result<Expense, AppError> {
        self.decide(id, None, actor).await
    }
    pub async fn reject(&self, id: Uuid, reason: &str, actor: Uuid) -> Result<Expense, AppError> {
        if reason.trim().is_empty() {
            return Err(AppError::BadRequest("Rejection reason is required".into()));
        }
        self.decide(id, Some(reason.trim().into()), actor).await
    }
    async fn decide(
        &self,
        id: Uuid,
        reason: Option<String>,
        actor: Uuid,
    ) -> Result<Expense, AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let old = Self::get(c, id, false).await?;
                    if old.approval_status == "approved" || old.approval_status == "rejected" && reason.is_some() {
                        return Err(AppError::Conflict("Expense decision is already recorded".into()));
                    }
                    let status = if reason.is_some() { "rejected" } else { "approved" };
                    let mut shift = old.shift_id;
                    let mut entry = None;
                    if status == "approved" && matches!(old.payment_method.as_str(), "cash" | "split_payment") {
                        if shift.is_none() {
                            let value: Option<String> = sqlx::query_scalar("SELECT id FROM shifts WHERE user_id=? AND status='active'").bind(actor.to_string()).fetch_optional(&mut *c).await?;
                            shift = value.map(|x| Uuid::parse_str(&x).map_err(|e| AppError::Internal(e.to_string()))).transpose()?;
                        }
                        let shift = shift.ok_or_else(|| AppError::conflict_code("CASH_REGISTER_REQUIRED", None))?;
                        let register = TenantCashRegisterRepository::by_shift(c, shift).await?.filter(|x| x.status == "open").ok_or_else(|| AppError::conflict_code("CASH_REGISTER_REQUIRED", None))?;
                        let amount: i64 = sqlx::query_scalar("SELECT amount FROM expenses WHERE id=?").bind(id.to_string()).fetch_one(&mut *c).await?;
                        entry = Some(TenantCashRegisterRepository::entry_on(c, register.id, "cash_out", amount, Some(format!("Expense: {}", old.description.as_deref().unwrap_or("N/A"))), Some(id), Some("expense".into()), actor, false).await?.id);
                    }
                    let ts = now()?;
                    sqlx::query("UPDATE expenses SET approval_status=?,approved_by=?,approved_at=?,rejection_reason=?,shift_id=?,cash_register_entry_id=COALESCE(?,cash_register_entry_id),updated_by=?,updated_at=? WHERE id=?").bind(status).bind(actor.to_string()).bind(&ts).bind(reason).bind(shift.map(|x| x.to_string())).bind(entry.map(|x| x.to_string())).bind(actor.to_string()).bind(&ts).bind(id.to_string()).execute(&mut *c).await?;
                    let row = Self::get(c, id, false).await?;
                    let venue = Self::scope(c, row.shift_id).await?;
                    event(c, "expense", id, "expense.status_changed", venue, false, json!(row)).await?;
                    event(c, "expense", id, "approval.decided", venue, false, json!({"expense_id":id,"entity_type":"expense","status":status,"requestedBy":old.created_by})).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn get_summary(&self) -> Result<Vec<ExpenseSummaryDto>, AppError> {
        Err(AppError::Api {
            code: "ANALYTICS_UNAVAILABLE".into(),
            status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
            details: None,
        })
    }
}

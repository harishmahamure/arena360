use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CashRegister, CashRegisterEntry, CashRegisterFilterDto, CloseCashRegisterDto,
    CreateCashRegisterEntryDto, OpenCashRegisterDto,
};

pub struct CashRegisterRepository {
    pool: PgPool,
}

impl CashRegisterRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id,
               "shiftId" as shift_id,
               "openedBy" as opened_by,
               "closedBy" as closed_by,
               "openingBalance"::float8 as opening_balance,
               "openingDenominations" as opening_denominations,
               "closingBalance"::float8 as closing_balance,
               "closingDenominations" as closing_denominations,
               "expectedClosing"::float8 as expected_closing,
               variance::float8 as variance,
               status,
               notes,
               "reconciledBy" as reconciled_by,
               "reconciledAt" as reconciled_at,
               "reconciliationNotes" as reconciliation_notes,
               "createdBy" as created_by,
               "updatedBy" as updated_by,
               "createdAt" as created_at,
               "updatedAt" as updated_at
        FROM cash_registers
    "#;

    const ENTRY_AGGREGATES_JOIN: &'static str = r#"
               LEFT JOIN (
                   SELECT e."cashRegisterId",
                          SUM(e.amount::float8) FILTER (WHERE e."entryType" = 'cash_in') AS total_cash_in,
                          SUM(e.amount::float8) FILTER (WHERE e."entryType" = 'cash_out') AS total_cash_out,
                          SUM(e.amount::float8) FILTER (
                              WHERE e."entryType" = 'cash_out'
                                AND e."referenceType" = 'cash_deposit'
                                AND d.status = 'approved'
                          ) AS total_deposited
                   FROM cash_register_entries e
                   LEFT JOIN cash_deposits d ON d.id = e."referenceId"
                   GROUP BY e."cashRegisterId"
               ) agg ON agg."cashRegisterId" = cr.id"#;

    const SELECT_WITH_TOTALS: &'static str = r#"
        SELECT cr.id,
               cr."shiftId" as shift_id,
               cr."openedBy" as opened_by,
               cr."closedBy" as closed_by,
               cr."openingBalance"::float8 as opening_balance,
               cr."openingDenominations" as opening_denominations,
               cr."closingBalance"::float8 as closing_balance,
               cr."closingDenominations" as closing_denominations,
               cr."expectedClosing"::float8 as expected_closing,
               cr.variance::float8 as variance,
               cr.status,
               cr.notes,
               cr."reconciledBy" as reconciled_by,
               cr."reconciledAt" as reconciled_at,
               cr."reconciliationNotes" as reconciliation_notes,
               cr."createdBy" as created_by,
               cr."updatedBy" as updated_by,
               cr."createdAt" as created_at,
               cr."updatedAt" as updated_at,
               COALESCE(agg.total_cash_in, 0)::float8 as total_cash_in,
               COALESCE(agg.total_cash_out, 0)::float8 as total_cash_out,
               COALESCE(agg.total_deposited, 0)::float8 as total_deposited
        FROM cash_registers cr
    "#;

    const ENTRY_SELECT: &'static str = r#"
        SELECT id,
               "cashRegisterId" as cash_register_id,
               "entryType" as entry_type,
               amount::float8 as amount,
               reason,
               "referenceId" as reference_id,
               "referenceType" as reference_type,
               "createdBy" as created_by,
               "createdAt" as created_at
        FROM cash_register_entries
    "#;

    pub async fn open_register(
        &self,
        dto: &OpenCashRegisterDto,
        actor_id: Uuid,
    ) -> Result<CashRegister, AppError> {
        let register = sqlx::query_as::<_, CashRegister>(
            r#"
            INSERT INTO cash_registers (
                id, "shiftId", "openedBy", "openingBalance", "openingDenominations",
                notes, status, "createdBy", "updatedBy", "createdAt", "updatedAt"
            )
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, 'open', $2, $2, NOW(), NOW())
            RETURNING id,
                      "shiftId" as shift_id,
                      "openedBy" as opened_by,
                      "closedBy" as closed_by,
                      "openingBalance"::float8 as opening_balance,
                      "openingDenominations" as opening_denominations,
                      "closingBalance"::float8 as closing_balance,
                      "closingDenominations" as closing_denominations,
                      "expectedClosing"::float8 as expected_closing,
                      variance::float8 as variance,
                      status,
                      notes,
                      "reconciledBy" as reconciled_by,
                      "reconciledAt" as reconciled_at,
                      "reconciliationNotes" as reconciliation_notes,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(dto.shift_id)
        .bind(actor_id)
        .bind(dto.opening_balance)
        .bind(&dto.opening_denominations)
        .bind(&dto.notes)
        .fetch_one(&self.pool)
        .await?;

        Ok(register)
    }

    pub async fn close_register(
        &self,
        id: Uuid,
        dto: &CloseCashRegisterDto,
        actor_id: Uuid,
    ) -> Result<CashRegister, AppError> {
        let expected = self.calculate_expected_closing(id).await?;
        let variance = dto.closing_balance - expected;

        let register = sqlx::query_as::<_, CashRegister>(
            r#"
            UPDATE cash_registers SET
                "closingBalance" = $2,
                "closingDenominations" = $3,
                "expectedClosing" = $4,
                variance = $5,
                status = 'closed',
                "closedBy" = $6,
                notes = COALESCE($7, notes),
                "updatedBy" = $6,
                "updatedAt" = NOW()
            WHERE id = $1 AND status = 'open'
            RETURNING id,
                      "shiftId" as shift_id,
                      "openedBy" as opened_by,
                      "closedBy" as closed_by,
                      "openingBalance"::float8 as opening_balance,
                      "openingDenominations" as opening_denominations,
                      "closingBalance"::float8 as closing_balance,
                      "closingDenominations" as closing_denominations,
                      "expectedClosing"::float8 as expected_closing,
                      variance::float8 as variance,
                      status,
                      notes,
                      "reconciledBy" as reconciled_by,
                      "reconciledAt" as reconciled_at,
                      "reconciliationNotes" as reconciliation_notes,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(id)
        .bind(dto.closing_balance)
        .bind(&dto.closing_denominations)
        .bind(expected)
        .bind(variance)
        .bind(actor_id)
        .bind(&dto.notes)
        .fetch_optional(&self.pool)
        .await?;

        register
            .ok_or_else(|| AppError::NotFound(format!("Open cash register with ID {id} not found")))
    }

    pub async fn reconcile(
        &self,
        id: Uuid,
        notes: Option<String>,
        actor_id: Uuid,
    ) -> Result<CashRegister, AppError> {
        let register = sqlx::query_as::<_, CashRegister>(
            r#"
            UPDATE cash_registers SET
                status = 'reconciled',
                "reconciledBy" = $2,
                "reconciledAt" = NOW(),
                "reconciliationNotes" = COALESCE($3, "reconciliationNotes"),
                "updatedBy" = $2,
                "updatedAt" = NOW()
            WHERE id = $1 AND status = 'closed'
            RETURNING id,
                      "shiftId" as shift_id,
                      "openedBy" as opened_by,
                      "closedBy" as closed_by,
                      "openingBalance"::float8 as opening_balance,
                      "openingDenominations" as opening_denominations,
                      "closingBalance"::float8 as closing_balance,
                      "closingDenominations" as closing_denominations,
                      "expectedClosing"::float8 as expected_closing,
                      variance::float8 as variance,
                      status,
                      notes,
                      "reconciledBy" as reconciled_by,
                      "reconciledAt" as reconciled_at,
                      "reconciliationNotes" as reconciliation_notes,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(id)
        .bind(actor_id)
        .bind(notes)
        .fetch_optional(&self.pool)
        .await?;

        register.ok_or_else(|| {
            AppError::NotFound(format!("Closed cash register with ID {id} not found"))
        })
    }

    pub async fn update_opening_balance(
        &self,
        id: Uuid,
        opening_balance: f64,
        opening_denominations: Option<serde_json::Value>,
        actor_id: Uuid,
    ) -> Result<CashRegister, AppError> {
        let register = sqlx::query_as::<_, CashRegister>(
            r#"
            UPDATE cash_registers SET
                "openingBalance" = $2,
                "openingDenominations" = $3,
                "updatedBy" = $4,
                "updatedAt" = NOW()
            WHERE id = $1 AND status = 'open'
            RETURNING id,
                      "shiftId" as shift_id,
                      "openedBy" as opened_by,
                      "closedBy" as closed_by,
                      "openingBalance"::float8 as opening_balance,
                      "openingDenominations" as opening_denominations,
                      "closingBalance"::float8 as closing_balance,
                      "closingDenominations" as closing_denominations,
                      "expectedClosing"::float8 as expected_closing,
                      variance::float8 as variance,
                      status,
                      notes,
                      "reconciledBy" as reconciled_by,
                      "reconciledAt" as reconciled_at,
                      "reconciliationNotes" as reconciliation_notes,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(id)
        .bind(opening_balance)
        .bind(opening_denominations)
        .bind(actor_id)
        .fetch_optional(&self.pool)
        .await?;

        register
            .ok_or_else(|| AppError::NotFound(format!("Open cash register with ID {id} not found")))
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<CashRegister>, AppError> {
        let query = format!(
            "{}{} WHERE cr.id = $1",
            Self::SELECT_WITH_TOTALS,
            Self::ENTRY_AGGREGATES_JOIN
        );
        let register = sqlx::query_as::<_, CashRegister>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(register)
    }

    pub async fn find_by_shift(&self, shift_id: Uuid) -> Result<Option<CashRegister>, AppError> {
        let query = format!(
            "{} WHERE \"shiftId\" = $1 ORDER BY \"createdAt\" DESC LIMIT 1",
            Self::SELECT
        );
        let register = sqlx::query_as::<_, CashRegister>(&query)
            .bind(shift_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(register)
    }

    pub async fn list(
        &self,
        filters: &CashRegisterFilterDto,
    ) -> Result<PaginationResult<CashRegister>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(format!(
            "{}{} WHERE 1=1",
            Self::SELECT_WITH_TOTALS,
            Self::ENTRY_AGGREGATES_JOIN
        ));

        Self::apply_filters(&mut builder, filters);

        let sort_by = filters.sort_by.as_deref().unwrap_or("createdAt");
        let sort_col = match sort_by {
            "openingBalance" => "cr.\"openingBalance\"",
            "status" => "cr.status",
            "updatedAt" => "cr.\"updatedAt\"",
            _ => "cr.\"createdAt\"",
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

        let rows = builder
            .build_query_as::<CashRegister>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM cash_registers cr WHERE 1=1");
        Self::apply_filters(&mut count_builder, filters);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(rows, total.0, page, limit))
    }

    fn apply_filters(builder: &mut QueryBuilder<Postgres>, filters: &CashRegisterFilterDto) {
        if let Some(shift_id) = filters.shift_id {
            builder.push(" AND cr.\"shiftId\" = ");
            builder.push_bind(shift_id);
        }
        if let Some(ref status) = filters.status {
            builder.push(" AND cr.status = ");
            builder.push_bind(status.clone());
        }
        if let Some(opened_by) = filters.opened_by {
            builder.push(" AND cr.\"openedBy\" = ");
            builder.push_bind(opened_by);
        }
    }

    pub async fn add_entry(
        &self,
        register_id: Uuid,
        dto: &CreateCashRegisterEntryDto,
        actor_id: Uuid,
    ) -> Result<CashRegisterEntry, AppError> {
        let entry = sqlx::query_as::<_, CashRegisterEntry>(
            r#"
            INSERT INTO cash_register_entries (
                id, "cashRegisterId", "entryType", amount, reason,
                "referenceId", "referenceType", "createdBy", "createdAt"
            )
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, NOW())
            RETURNING id,
                      "cashRegisterId" as cash_register_id,
                      "entryType" as entry_type,
                      amount::float8 as amount,
                      reason,
                      "referenceId" as reference_id,
                      "referenceType" as reference_type,
                      "createdBy" as created_by,
                      "createdAt" as created_at
            "#,
        )
        .bind(register_id)
        .bind(&dto.entry_type)
        .bind(dto.amount)
        .bind(&dto.reason)
        .bind(dto.reference_id)
        .bind(&dto.reference_type)
        .bind(actor_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(entry)
    }

    pub async fn find_last_closed_by_user(
        &self,
        user_id: Uuid,
    ) -> Result<Option<CashRegister>, AppError> {
        let register = sqlx::query_as::<_, CashRegister>(
            r#"
            SELECT cr.id,
                   cr."shiftId" as shift_id,
                   cr."openedBy" as opened_by,
                   cr."closedBy" as closed_by,
                   cr."openingBalance"::float8 as opening_balance,
                   cr."openingDenominations" as opening_denominations,
                   cr."closingBalance"::float8 as closing_balance,
                   cr."closingDenominations" as closing_denominations,
                   cr."expectedClosing"::float8 as expected_closing,
                   cr.variance::float8 as variance,
                   cr.status,
                   cr.notes,
                   cr."reconciledBy" as reconciled_by,
                   cr."reconciledAt" as reconciled_at,
                   cr."reconciliationNotes" as reconciliation_notes,
                   cr."createdBy" as created_by,
                   cr."updatedBy" as updated_by,
                   cr."createdAt" as created_at,
                   cr."updatedAt" as updated_at
            FROM cash_registers cr
            INNER JOIN shifts s ON s.id = cr."shiftId"
            WHERE s."userId" = $1 AND cr.status IN ('closed', 'reconciled')
            ORDER BY cr."updatedAt" DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(register)
    }

    /// Most recently closed/reconciled register site-wide (single drawer carry-forward).
    pub async fn find_last_closed_register(&self) -> Result<Option<CashRegister>, AppError> {
        let query = format!(
            r#"{base} WHERE status IN ('closed', 'reconciled') ORDER BY "updatedAt" DESC LIMIT 1"#,
            base = Self::SELECT
        );
        let register = sqlx::query_as::<_, CashRegister>(&query)
            .fetch_optional(&self.pool)
            .await?;
        Ok(register)
    }

    pub async fn list_entries(
        &self,
        register_id: Uuid,
    ) -> Result<Vec<CashRegisterEntry>, AppError> {
        let query = format!(
            "{} WHERE \"cashRegisterId\" = $1 ORDER BY \"createdAt\" ASC",
            Self::ENTRY_SELECT
        );
        let entries = sqlx::query_as::<_, CashRegisterEntry>(&query)
            .bind(register_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(entries)
    }

    pub async fn sum_entries_by_type(
        &self,
        register_id: Uuid,
        entry_type: &str,
    ) -> Result<f64, AppError> {
        let row: (Option<f64>,) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(amount::float8), 0.0) as total
            FROM cash_register_entries
            WHERE "cashRegisterId" = $1 AND "entryType" = $2
            "#,
        )
        .bind(register_id)
        .bind(entry_type)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0.unwrap_or(0.0))
    }

    pub async fn sum_deposit_entries(&self, register_id: Uuid) -> Result<f64, AppError> {
        let row: (Option<f64>,) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(e.amount::float8), 0.0)
            FROM cash_register_entries e
            JOIN cash_deposits d ON d.id = e."referenceId"
            WHERE e."cashRegisterId" = $1
              AND e."entryType" = 'cash_out'
              AND e."referenceType" = 'cash_deposit'
              AND d.status = 'approved'
            "#,
        )
        .bind(register_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0.unwrap_or(0.0))
    }

    /// Sum of all deposit cash_out entries (pending + approved) on a register.
    /// Used for carry-forward: closing balance is counted before deposit removal.
    pub async fn sum_all_deposit_cash_out(&self, register_id: Uuid) -> Result<f64, AppError> {
        let row: (Option<f64>,) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(e.amount::float8), 0.0)
            FROM cash_register_entries e
            WHERE e."cashRegisterId" = $1
              AND e."entryType" = 'cash_out'
              AND e."referenceType" = 'cash_deposit'
            "#,
        )
        .bind(register_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0.unwrap_or(0.0))
    }

    pub async fn get_expected_closing(&self, register_id: Uuid) -> Result<f64, AppError> {
        self.calculate_expected_closing(register_id).await
    }

    /// Recompute stored expected closing and variance after deposit approval/rejection.
    pub async fn recalculate_closed_register_totals(
        &self,
        register_id: Uuid,
    ) -> Result<Option<CashRegister>, AppError> {
        let register = self.find_by_id(register_id).await?.ok_or_else(|| {
            AppError::NotFound(format!("Cash register with ID {register_id} not found"))
        })?;

        if register.status != "closed" && register.status != "reconciled" {
            return Ok(None);
        }

        let Some(closing) = register.closing_balance else {
            return Ok(None);
        };

        let ledger_expected = self.compute_ledger_expected(register_id, &register).await?;
        let approved_deposits = self.sum_deposit_entries(register_id).await?;
        let variance = closing - ledger_expected - approved_deposits;

        let updated = sqlx::query_as::<_, CashRegister>(
            r#"
            UPDATE cash_registers SET
                "expectedClosing" = $2,
                variance = $3,
                "updatedAt" = NOW()
            WHERE id = $1
            RETURNING id,
                      "shiftId" as shift_id,
                      "openedBy" as opened_by,
                      "closedBy" as closed_by,
                      "openingBalance"::float8 as opening_balance,
                      "openingDenominations" as opening_denominations,
                      "closingBalance"::float8 as closing_balance,
                      "closingDenominations" as closing_denominations,
                      "expectedClosing"::float8 as expected_closing,
                      variance::float8 as variance,
                      status,
                      notes,
                      "reconciledBy" as reconciled_by,
                      "reconciledAt" as reconciled_at,
                      "reconciliationNotes" as reconciliation_notes,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(register_id)
        .bind(ledger_expected)
        .bind(variance)
        .fetch_optional(&self.pool)
        .await?;

        Ok(updated)
    }

    async fn compute_ledger_expected(
        &self,
        register_id: Uuid,
        register: &CashRegister,
    ) -> Result<f64, AppError> {
        let cash_in = self.sum_entries_by_type(register_id, "cash_in").await?;
        let cash_out = self.sum_entries_by_type(register_id, "cash_out").await?;
        let all_deposits = self.sum_all_deposit_cash_out(register_id).await?;

        // Pending and approved deposit cash_out stay in the drawer until approval;
        // exclude all deposit withdrawals from the ledger reduction.
        Ok(register.opening_balance + cash_in - (cash_out - all_deposits))
    }

    async fn calculate_expected_closing(&self, register_id: Uuid) -> Result<f64, AppError> {
        let register = self.find_by_id(register_id).await?.ok_or_else(|| {
            AppError::NotFound(format!("Cash register with ID {register_id} not found"))
        })?;

        self.compute_ledger_expected(register_id, &register).await
    }
}

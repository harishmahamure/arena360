use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{CashDeposit, CashDepositFilterDto, InitiateDepositDto};

pub struct CashDepositRepository {
    pool: PgPool,
}

impl CashDepositRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id,
               "cashRegisterId" as cash_register_id,
               "shiftId" as shift_id,
               "initiatedBy" as initiated_by,
               "approvedBy" as approved_by,
               amount::float8 as amount,
               denominations,
               "depositType" as deposit_type,
               status,
               "approvedAt" as approved_at,
               "rejectionReason" as rejection_reason,
               notes,
               "createdAt" as created_at,
               "updatedAt" as updated_at
        FROM cash_deposits
    "#;

    pub async fn create(
        &self,
        dto: &InitiateDepositDto,
        initiated_by: Uuid,
    ) -> Result<CashDeposit, AppError> {
        let deposit = sqlx::query_as::<_, CashDeposit>(
            r#"
            INSERT INTO cash_deposits (
                id, "cashRegisterId", "shiftId", "initiatedBy",
                amount, denominations, notes, status, "createdAt", "updatedAt"
            )
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, 'pending', NOW(), NOW())
            RETURNING id,
                      "cashRegisterId" as cash_register_id,
                      "shiftId" as shift_id,
                      "initiatedBy" as initiated_by,
                      "approvedBy" as approved_by,
                      amount::float8 as amount,
                      denominations,
                      "depositType" as deposit_type,
                      status,
                      "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason,
                      notes,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(dto.cash_register_id)
        .bind(dto.shift_id)
        .bind(initiated_by)
        .bind(dto.amount)
        .bind(&dto.denominations)
        .bind(&dto.notes)
        .fetch_one(&self.pool)
        .await?;

        Ok(deposit)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<CashDeposit>, AppError> {
        let query = format!("{} WHERE id = $1", Self::SELECT);
        let deposit = sqlx::query_as::<_, CashDeposit>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(deposit)
    }

    pub async fn approve(
        &self,
        id: Uuid,
        deposit_type: &str,
        approved_by: Uuid,
    ) -> Result<CashDeposit, AppError> {
        let deposit = sqlx::query_as::<_, CashDeposit>(
            r#"
            UPDATE cash_deposits SET
                status = 'approved',
                "depositType" = $2,
                "approvedBy" = $3,
                "approvedAt" = NOW(),
                "updatedAt" = NOW()
            WHERE id = $1 AND status = 'pending'
            RETURNING id,
                      "cashRegisterId" as cash_register_id,
                      "shiftId" as shift_id,
                      "initiatedBy" as initiated_by,
                      "approvedBy" as approved_by,
                      amount::float8 as amount,
                      denominations,
                      "depositType" as deposit_type,
                      status,
                      "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason,
                      notes,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(id)
        .bind(deposit_type)
        .bind(approved_by)
        .fetch_optional(&self.pool)
        .await?;

        deposit.ok_or_else(|| AppError::NotFound(format!("Pending deposit with ID {id} not found")))
    }

    pub async fn reject(
        &self,
        id: Uuid,
        rejection_reason: &str,
        approved_by: Uuid,
    ) -> Result<CashDeposit, AppError> {
        let deposit = sqlx::query_as::<_, CashDeposit>(
            r#"
            UPDATE cash_deposits SET
                status = 'rejected',
                "rejectionReason" = $2,
                "approvedBy" = $3,
                "approvedAt" = NOW(),
                "updatedAt" = NOW()
            WHERE id = $1 AND status = 'pending'
            RETURNING id,
                      "cashRegisterId" as cash_register_id,
                      "shiftId" as shift_id,
                      "initiatedBy" as initiated_by,
                      "approvedBy" as approved_by,
                      amount::float8 as amount,
                      denominations,
                      "depositType" as deposit_type,
                      status,
                      "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason,
                      notes,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(id)
        .bind(rejection_reason)
        .bind(approved_by)
        .fetch_optional(&self.pool)
        .await?;

        deposit.ok_or_else(|| AppError::NotFound(format!("Pending deposit with ID {id} not found")))
    }

    pub async fn list(
        &self,
        filters: &CashDepositFilterDto,
    ) -> Result<PaginationResult<CashDeposit>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT id,
                      "cashRegisterId" as cash_register_id,
                      "shiftId" as shift_id,
                      "initiatedBy" as initiated_by,
                      "approvedBy" as approved_by,
                      amount::float8 as amount,
                      denominations,
                      "depositType" as deposit_type,
                      status,
                      "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason,
                      notes,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
               FROM cash_deposits WHERE 1=1"#,
        );

        Self::apply_filters(&mut builder, filters);

        let sort_by = filters.sort_by.as_deref().unwrap_or("createdAt");
        let sort_col = match sort_by {
            "amount" => "amount",
            "status" => "status",
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

        let rows = builder
            .build_query_as::<CashDeposit>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM cash_deposits WHERE 1=1");
        Self::apply_filters(&mut count_builder, filters);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(rows, total.0, page, limit))
    }

    fn apply_filters(builder: &mut QueryBuilder<Postgres>, filters: &CashDepositFilterDto) {
        if let Some(shift_id) = filters.shift_id {
            builder.push(" AND \"shiftId\" = ");
            builder.push_bind(shift_id);
        }
        if let Some(cash_register_id) = filters.cash_register_id {
            builder.push(" AND \"cashRegisterId\" = ");
            builder.push_bind(cash_register_id);
        }
        if let Some(status) = &filters.status {
            builder.push(" AND status = ");
            builder.push_bind(status.clone());
        }
        if let Some(initiated_by) = filters.initiated_by {
            builder.push(" AND \"initiatedBy\" = ");
            builder.push_bind(initiated_by);
        }
    }
}

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{CreateTransactionDto, Transaction, TransactionFilterDto, TransactionRow};

pub struct TransactionRepository {
    pub(crate) pool: PgPool,
}

impl TransactionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id,
               "playerId" as player_id,
               "transactionType"::text as transaction_type,
               "planId" as plan_id,
               "shiftId" as shift_id,
               amount::float8 as amount,
               "paidAmount"::float8 as paid_amount,
               "cashAmount"::float8 as cash_amount,
               "onlineAmount"::float8 as online_amount,
               "paymentMethod"::text as payment_method,
               "paymentStatus"::text as payment_status,
               notes,
               "transactionDate" as transaction_date,
               "createdBy" as created_by,
               "updatedBy" as updated_by,
               "createdAt" as created_at,
               "updatedAt" as updated_at,
               "deletedAt" as deleted_at
        FROM transactions
    "#;

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Transaction>, AppError> {
        let query = format!("{} WHERE id = $1 AND \"deletedAt\" IS NULL", Self::SELECT);
        let transaction = sqlx::query_as::<_, Transaction>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(transaction)
    }

    pub async fn list(
        &self,
        filters: &TransactionFilterDto,
    ) -> Result<PaginationResult<crate::models::TransactionResponse>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT t.id,
                      t."playerId" as player_id,
                      t."transactionType"::text as transaction_type,
                      t."planId" as plan_id,
                      t."shiftId" as shift_id,
                      t.amount::float8 as amount,
                      t."paidAmount"::float8 as paid_amount,
                      t."cashAmount"::float8 as cash_amount,
                      t."onlineAmount"::float8 as online_amount,
                      t."paymentMethod"::text as payment_method,
                      t."paymentStatus"::text as payment_status,
                      t.notes,
                      t."transactionDate" as transaction_date,
                      t."createdBy" as created_by,
                      t."updatedBy" as updated_by,
                      t."createdAt" as created_at,
                      t."updatedAt" as updated_at,
                      t."deletedAt" as deleted_at,
                      u.username as player_username,
                      u."firstName" as player_first_name,
                      u."lastName" as player_last_name,
                      p.name as plan_name,
                      p."planType"::text as plan_type,
                      p.price::float8 as plan_price
               FROM transactions t
               LEFT JOIN users u ON u.id = t."playerId" AND u."deletedAt" IS NULL
               LEFT JOIN plans p ON p.id = t."planId" AND p."deletedAt" IS NULL
               WHERE t."deletedAt" IS NULL"#,
        );

        Self::apply_filters(&mut builder, filters, "t");

        let sort_by = filters.sort_by.as_deref().unwrap_or("transactionDate");
        let sort_col = match sort_by {
            "amount" => "t.amount",
            "paymentStatus" => r#"t."paymentStatus""#,
            "createdAt" => r#"t."createdAt""#,
            _ => r#"t."transactionDate""#,
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
            .build_query_as::<TransactionRow>()
            .fetch_all(&self.pool)
            .await?;

        let transactions: Vec<crate::models::TransactionResponse> =
            rows.into_iter().map(|row| row.into_response()).collect();

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new(r#"SELECT COUNT(*) FROM transactions t WHERE t."deletedAt" IS NULL"#);
        Self::apply_filters(&mut count_builder, filters, "t");

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(transactions, total.0, page, limit))
    }

    fn apply_filters(
        builder: &mut QueryBuilder<Postgres>,
        filters: &TransactionFilterDto,
        table_alias: &str,
    ) {
        if let Some(player_id) = filters.player_id {
            builder.push(format!(r#" AND {table_alias}."playerId" = "#));
            builder.push_bind(player_id);
        }
        if let Some(transaction_type) = filters.transaction_type.clone() {
            builder.push(format!(r#" AND {table_alias}."transactionType"::text = "#));
            builder.push_bind(transaction_type);
        }
        if let Some(plan_id) = filters.plan_id {
            builder.push(format!(r#" AND {table_alias}."planId" = "#));
            builder.push_bind(plan_id);
        }
        if let Some(shift_id) = filters.shift_id {
            builder.push(format!(r#" AND {table_alias}."shiftId" = "#));
            builder.push_bind(shift_id);
        }
        if let Some(payment_method) = filters.payment_method.clone() {
            builder.push(format!(r#" AND {table_alias}."paymentMethod"::text = "#));
            builder.push_bind(payment_method);
        }
        if let Some(payment_status) = filters.payment_status.clone() {
            builder.push(format!(r#" AND {table_alias}."paymentStatus"::text = "#));
            builder.push_bind(payment_status);
        }
        if let Some(from) = filters.transaction_date_from {
            builder.push(format!(r#" AND {table_alias}."transactionDate" >= "#));
            builder.push_bind(from);
        }
        if let Some(to) = filters.transaction_date_to {
            builder.push(format!(r#" AND {table_alias}."transactionDate" <= "#));
            builder.push_bind(to);
        }
        if let Some(min_amount) = filters.min_amount {
            builder.push(format!(" AND {table_alias}.amount >= "));
            builder.push_bind(min_amount);
        }
        if let Some(max_amount) = filters.max_amount {
            builder.push(format!(" AND {table_alias}.amount <= "));
            builder.push_bind(max_amount);
        }
    }

    pub async fn create(
        &self,
        dto: &CreateTransactionDto,
        amount: f64,
        transaction_date: DateTime<Utc>,
        payment_status: &str,
        actor_id: Option<Uuid>,
    ) -> Result<Transaction, AppError> {
        let transaction = sqlx::query_as::<_, Transaction>(
            r#"
            INSERT INTO transactions (
                id, "playerId", "transactionType", "planId", "shiftId", amount,
                "paidAmount", "cashAmount", "onlineAmount", "paymentMethod", "paymentStatus",
                notes, "transactionDate", "createdBy", "updatedBy", "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2::transactions_transactiontype_enum, $3, $4, $5,
                0, $6, $7, $8::transactions_paymentmethod_enum, $9::transactions_paymentstatus_enum,
                $10, $11, $12, $12, NOW(), NOW()
            )
            RETURNING id,
                      "playerId" as player_id,
                      "transactionType"::text as transaction_type,
                      "planId" as plan_id,
                      "shiftId" as shift_id,
                      amount::float8 as amount,
                      "paidAmount"::float8 as paid_amount,
                      "cashAmount"::float8 as cash_amount,
                      "onlineAmount"::float8 as online_amount,
                      "paymentMethod"::text as payment_method,
                      "paymentStatus"::text as payment_status,
                      notes,
                      "transactionDate" as transaction_date,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(dto.player_id)
        .bind(&dto.transaction_type)
        .bind(dto.plan_id)
        .bind(dto.shift_id)
        .bind(amount)
        .bind(dto.cash_amount)
        .bind(dto.online_amount)
        .bind(&dto.payment_method)
        .bind(payment_status)
        .bind(&dto.notes)
        .bind(transaction_date)
        .bind(actor_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(transaction)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: &crate::models::UpdateTransactionDto,
        actor_id: Option<Uuid>,
    ) -> Result<Transaction, AppError> {
        let transaction = sqlx::query_as::<_, Transaction>(
            r#"
            UPDATE transactions SET
                "paymentStatus" = COALESCE($2::transactions_paymentstatus_enum, "paymentStatus"),
                notes = COALESCE($3, notes),
                "updatedBy" = COALESCE($4, "updatedBy"),
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id,
                      "playerId" as player_id,
                      "transactionType"::text as transaction_type,
                      "planId" as plan_id,
                      "shiftId" as shift_id,
                      amount::float8 as amount,
                      "paidAmount"::float8 as paid_amount,
                      "cashAmount"::float8 as cash_amount,
                      "onlineAmount"::float8 as online_amount,
                      "paymentMethod"::text as payment_method,
                      "paymentStatus"::text as payment_status,
                      notes,
                      "transactionDate" as transaction_date,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .bind(&dto.payment_status)
        .bind(&dto.notes)
        .bind(actor_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Transaction with ID {id} not found")))?;

        Ok(transaction)
    }

    pub async fn create_in_tx(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        dto: &CreateTransactionDto,
        amount: f64,
        transaction_date: DateTime<Utc>,
        payment_status: &str,
        actor_id: Option<Uuid>,
    ) -> Result<Transaction, AppError> {
        let transaction = sqlx::query_as::<_, Transaction>(
            r#"
            INSERT INTO transactions (
                id, "playerId", "transactionType", "planId", "shiftId", amount,
                "paidAmount", "cashAmount", "onlineAmount", "paymentMethod", "paymentStatus",
                notes, "transactionDate", "createdBy", "updatedBy", "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2::transactions_transactiontype_enum, $3, $4, $5,
                0, $6, $7, $8::transactions_paymentmethod_enum, $9::transactions_paymentstatus_enum,
                $10, $11, $12, $12, NOW(), NOW()
            )
            RETURNING id,
                      "playerId" as player_id,
                      "transactionType"::text as transaction_type,
                      "planId" as plan_id,
                      "shiftId" as shift_id,
                      amount::float8 as amount,
                      "paidAmount"::float8 as paid_amount,
                      "cashAmount"::float8 as cash_amount,
                      "onlineAmount"::float8 as online_amount,
                      "paymentMethod"::text as payment_method,
                      "paymentStatus"::text as payment_status,
                      notes,
                      "transactionDate" as transaction_date,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(dto.player_id)
        .bind(&dto.transaction_type)
        .bind(dto.plan_id)
        .bind(dto.shift_id)
        .bind(amount)
        .bind(dto.cash_amount)
        .bind(dto.online_amount)
        .bind(&dto.payment_method)
        .bind(payment_status)
        .bind(&dto.notes)
        .bind(transaction_date)
        .bind(actor_id)
        .fetch_one(&mut **tx)
        .await?;

        Ok(transaction)
    }

    pub async fn plan_price(&self, plan_id: Uuid) -> Result<Option<f64>, AppError> {
        let row: Option<(f64,)> = sqlx::query_as(
            r#"SELECT price::float8 FROM plans WHERE id = $1 AND "deletedAt" IS NULL"#,
        )
        .bind(plan_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.0))
    }
}

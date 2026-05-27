use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{CreateTransactionDto, Transaction, TransactionFilterDto};

pub struct TransactionRepository {
    pool: PgPool,
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
               amount::float8 as amount,
               "cashAmount"::float8 as cash_amount,
               "onlineAmount"::float8 as online_amount,
               "paymentMethod"::text as payment_method,
               "paymentStatus"::text as payment_status,
               notes,
               "transactionDate" as transaction_date,
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
    ) -> Result<PaginationResult<Transaction>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, \"playerId\" as player_id, \"transactionType\"::text as transaction_type, \
             \"planId\" as plan_id, amount::float8 as amount, \"cashAmount\"::float8 as cash_amount, \
             \"onlineAmount\"::float8 as online_amount, \"paymentMethod\"::text as payment_method, \
             \"paymentStatus\"::text as payment_status, notes, \"transactionDate\" as transaction_date, \
             \"createdAt\" as created_at, \"updatedAt\" as updated_at, \"deletedAt\" as deleted_at \
             FROM transactions WHERE \"deletedAt\" IS NULL",
        );

        Self::apply_filters(&mut builder, filters);

        let sort_by = filters.sort_by.as_deref().unwrap_or("transactionDate");
        let sort_col = match sort_by {
            "amount" => "amount",
            "paymentStatus" => "\"paymentStatus\"",
            "createdAt" => "\"createdAt\"",
            _ => "\"transactionDate\"",
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

        let transactions = builder
            .build_query_as::<Transaction>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM transactions WHERE \"deletedAt\" IS NULL");
        Self::apply_filters(&mut count_builder, filters);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(transactions, total.0, page, limit))
    }

    fn apply_filters(builder: &mut QueryBuilder<Postgres>, filters: &TransactionFilterDto) {
        if let Some(player_id) = filters.player_id {
            builder.push(" AND \"playerId\" = ");
            builder.push_bind(player_id);
        }
        if let Some(transaction_type) = filters.transaction_type.clone() {
            builder.push(" AND \"transactionType\"::text = ");
            builder.push_bind(transaction_type);
        }
        if let Some(plan_id) = filters.plan_id {
            builder.push(" AND \"planId\" = ");
            builder.push_bind(plan_id);
        }
        if let Some(payment_method) = filters.payment_method.clone() {
            builder.push(" AND \"paymentMethod\"::text = ");
            builder.push_bind(payment_method);
        }
        if let Some(payment_status) = filters.payment_status.clone() {
            builder.push(" AND \"paymentStatus\"::text = ");
            builder.push_bind(payment_status);
        }
        if let Some(from) = filters.transaction_date_from {
            builder.push(" AND \"transactionDate\" >= ");
            builder.push_bind(from);
        }
        if let Some(to) = filters.transaction_date_to {
            builder.push(" AND \"transactionDate\" <= ");
            builder.push_bind(to);
        }
        if let Some(min_amount) = filters.min_amount {
            builder.push(" AND amount >= ");
            builder.push_bind(min_amount);
        }
        if let Some(max_amount) = filters.max_amount {
            builder.push(" AND amount <= ");
            builder.push_bind(max_amount);
        }
    }

    pub async fn create(
        &self,
        dto: &CreateTransactionDto,
        amount: f64,
        transaction_date: DateTime<Utc>,
        payment_status: &str,
    ) -> Result<Transaction, AppError> {
        let transaction = sqlx::query_as::<_, Transaction>(
            r#"
            INSERT INTO transactions (
                id, "playerId", "transactionType", "planId", amount,
                "cashAmount", "onlineAmount", "paymentMethod", "paymentStatus",
                notes, "transactionDate", "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2::transactions_transactiontype_enum, $3, $4,
                $5, $6, $7::transactions_paymentmethod_enum, $8::transactions_paymentstatus_enum,
                $9, $10, NOW(), NOW()
            )
            RETURNING id,
                      "playerId" as player_id,
                      "transactionType"::text as transaction_type,
                      "planId" as plan_id,
                      amount::float8 as amount,
                      "cashAmount"::float8 as cash_amount,
                      "onlineAmount"::float8 as online_amount,
                      "paymentMethod"::text as payment_method,
                      "paymentStatus"::text as payment_status,
                      notes,
                      "transactionDate" as transaction_date,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(dto.player_id)
        .bind(&dto.transaction_type)
        .bind(dto.plan_id)
        .bind(amount)
        .bind(dto.cash_amount)
        .bind(dto.online_amount)
        .bind(&dto.payment_method)
        .bind(payment_status)
        .bind(&dto.notes)
        .bind(transaction_date)
        .fetch_one(&self.pool)
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

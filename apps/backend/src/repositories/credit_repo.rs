use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CreditAccountFilterDto, CreditPlayerRow, CreditSettlement, OutstandingTxnRow, SettleItemDto,
};

#[derive(sqlx::FromRow)]
struct CreditPlayerRowWithTotal {
    player_id: Uuid,
    username: String,
    first_name: Option<String>,
    last_name: Option<String>,
    phone_number: Option<String>,
    credit_limit: f64,
    outstanding: f64,
    available: f64,
    total_count: i64,
}

impl From<CreditPlayerRowWithTotal> for CreditPlayerRow {
    fn from(row: CreditPlayerRowWithTotal) -> Self {
        Self {
            player_id: row.player_id,
            username: row.username,
            first_name: row.first_name,
            last_name: row.last_name,
            phone_number: row.phone_number,
            credit_limit: row.credit_limit,
            outstanding: row.outstanding,
            available: row.available,
        }
    }
}

pub struct CreditRepository {
    pool: PgPool,
}

impl CreditRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_credit_limit(&self, player_id: Uuid) -> Result<Option<f64>, AppError> {
        let row: Option<(f64,)> = sqlx::query_as(
            r#"SELECT "creditLimit"::float8 FROM users WHERE id = $1 AND "deletedAt" IS NULL"#,
        )
        .bind(player_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn set_credit_limit(
        &self,
        player_id: Uuid,
        credit_limit: f64,
        actor_id: Option<Uuid>,
    ) -> Result<(), AppError> {
        let rows = sqlx::query(
            r#"UPDATE users SET "creditLimit" = $2, "updatedBy" = $3, "updatedAt" = NOW()
               WHERE id = $1 AND "deletedAt" IS NULL AND role = 'player'"#,
        )
        .bind(player_id)
        .bind(credit_limit)
        .bind(actor_id)
        .execute(&self.pool)
        .await?;

        if rows.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "Player with ID {player_id} not found"
            )));
        }
        Ok(())
    }

    pub async fn sum_outstanding(&self, player_id: Uuid) -> Result<f64, AppError> {
        let row: (Option<f64>,) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(amount - "paidAmount"), 0)::float8
            FROM transactions
            WHERE "playerId" = $1
              AND "paymentMethod" = 'credit'
              AND "paymentStatus" = 'credit'
              AND "deletedAt" IS NULL
            "#,
        )
        .bind(player_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0.unwrap_or(0.0))
    }

    pub async fn list_credit_players(
        &self,
        filters: &CreditAccountFilterDto,
    ) -> Result<PaginationResult<CreditPlayerRow>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"
            SELECT u.id as player_id,
                   u.username,
                   u."firstName" as first_name,
                   u."lastName" as last_name,
                   u."phoneNumber" as phone_number,
                   u."creditLimit"::float8 as credit_limit,
                   COALESCE(SUM(t.amount - t."paidAmount"), 0)::float8 as outstanding,
                   GREATEST(u."creditLimit" - COALESCE(SUM(t.amount - t."paidAmount"), 0), 0)::float8 as available,
                   COUNT(*) OVER() as total_count
            FROM users u
            INNER JOIN transactions t ON t."playerId" = u.id
                AND t."paymentMethod" = 'credit'
                AND t."paymentStatus" = 'credit'
                AND t."deletedAt" IS NULL
            WHERE u."deletedAt" IS NULL
              AND u.role = 'player'
            "#,
        );

        if let Some(search) = filters.search.as_ref().filter(|s| !s.is_empty()) {
            let pattern = format!("%{search}%");
            builder.push(" AND (u.username ILIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR u.\"firstName\" ILIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR u.\"lastName\" ILIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR u.\"phoneNumber\" ILIKE ");
            builder.push_bind(pattern);
            builder.push(")");
        }

        builder.push(" GROUP BY u.id HAVING COALESCE(SUM(t.amount - t.\"paidAmount\"), 0) > 0");

        let sort_by = filters.sort_by.as_deref().unwrap_or("outstanding");
        let sort_col = match sort_by {
            "username" => "u.username",
            "creditLimit" => "u.\"creditLimit\"",
            "available" => "available",
            _ => "outstanding",
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
            .build_query_as::<CreditPlayerRowWithTotal>()
            .fetch_all(&self.pool)
            .await?;

        let total = rows.first().map(|r| r.total_count).unwrap_or(0);
        let items: Vec<CreditPlayerRow> = rows.into_iter().map(Into::into).collect();

        Ok(PaginationResult::new(items, total, page, limit))
    }

    pub async fn list_outstanding_txns(
        &self,
        player_id: Uuid,
    ) -> Result<Vec<OutstandingTxnRow>, AppError> {
        let rows = sqlx::query_as::<_, OutstandingTxnRow>(
            r#"
            SELECT id as transaction_id,
                   "transactionType"::text as transaction_type,
                   amount::float8 as amount,
                   "paidAmount"::float8 as paid_amount,
                   (amount - "paidAmount")::float8 as remaining,
                   "paymentStatus"::text as payment_status,
                   "transactionDate" as transaction_date,
                   notes
            FROM transactions
            WHERE "playerId" = $1
              AND "paymentMethod" = 'credit'
              AND "paymentStatus" = 'credit'
              AND "deletedAt" IS NULL
              AND amount > "paidAmount"
            ORDER BY "transactionDate" ASC
            "#,
        )
        .bind(player_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_settlement(
        &self,
        player_id: Uuid,
        shift_id: Uuid,
        settled_by: Uuid,
        total: f64,
        payment_method: &str,
        cash_amount: Option<f64>,
        online_amount: Option<f64>,
        notes: Option<&str>,
        items: &[SettleItemDto],
    ) -> Result<CreditSettlement, AppError> {
        let mut tx = self.pool.begin().await?;

        let settlement = sqlx::query_as::<_, CreditSettlement>(
            r#"
            INSERT INTO credit_settlements (
                id, "playerId", "shiftId", "settledBy", amount,
                "paymentMethod", "cashAmount", "onlineAmount", notes,
                "settledAt", "createdBy", "updatedBy", "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2, $3, $4,
                $5, $6, $7, $8,
                NOW(), $3, $3, NOW(), NOW()
            )
            RETURNING id,
                      "playerId" as player_id,
                      "shiftId" as shift_id,
                      "settledBy" as settled_by,
                      amount::float8 as amount,
                      "paymentMethod" as payment_method,
                      "cashAmount"::float8 as cash_amount,
                      "onlineAmount"::float8 as online_amount,
                      notes,
                      "settledAt" as settled_at,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(player_id)
        .bind(shift_id)
        .bind(settled_by)
        .bind(total)
        .bind(payment_method)
        .bind(cash_amount)
        .bind(online_amount)
        .bind(notes)
        .fetch_one(&mut *tx)
        .await?;

        for item in items {
            let row: Option<(f64, f64)> = sqlx::query_as(
                r#"
                SELECT amount::float8, "paidAmount"::float8
                FROM transactions
                WHERE id = $1
                  AND "playerId" = $2
                  AND "paymentMethod" = 'credit'
                  AND "paymentStatus" = 'credit'
                  AND "deletedAt" IS NULL
                FOR UPDATE
                "#,
            )
            .bind(item.transaction_id)
            .bind(player_id)
            .fetch_optional(&mut *tx)
            .await?;

            let (amount, paid) = row.ok_or_else(|| {
                AppError::NotFound(format!(
                    "Credit transaction {} not found for player",
                    item.transaction_id
                ))
            })?;

            let remaining = amount - paid;
            if item.amount > remaining + 0.001 {
                return Err(AppError::BadRequest(format!(
                    "Settlement amount {} exceeds remaining {} for transaction {}",
                    item.amount, remaining, item.transaction_id
                )));
            }

            sqlx::query(
                r#"
                INSERT INTO credit_settlement_items ("settlementId", "transactionId", "amountApplied")
                VALUES ($1, $2, $3)
                "#,
            )
            .bind(settlement.id)
            .bind(item.transaction_id)
            .bind(item.amount)
            .execute(&mut *tx)
            .await?;

            let new_paid = paid + item.amount;
            let new_status = if new_paid >= amount - 0.001 {
                "completed"
            } else {
                "credit"
            };

            sqlx::query(
                r#"
                UPDATE transactions
                SET "paidAmount" = $2,
                    "paymentStatus" = $3::transactions_paymentstatus_enum,
                    "updatedBy" = $4,
                    "updatedAt" = NOW()
                WHERE id = $1
                "#,
            )
            .bind(item.transaction_id)
            .bind(new_paid)
            .bind(new_status)
            .bind(settled_by)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(settlement)
    }

    pub async fn is_eligible_player(&self, player_id: Uuid) -> Result<(bool, f64), AppError> {
        let row: Option<(bool, String, f64)> = sqlx::query_as(
            r#"
            SELECT "isActive", COALESCE(role, 'player'), "creditLimit"::float8
            FROM users
            WHERE id = $1 AND "deletedAt" IS NULL
            "#,
        )
        .bind(player_id)
        .fetch_optional(&self.pool)
        .await?;

        let (is_active, role, credit_limit) =
            row.ok_or_else(|| AppError::BadRequest("player not eligible for credit".to_string()))?;

        if !is_active || role != "player" {
            return Err(AppError::BadRequest(
                "player not eligible for credit".to_string(),
            ));
        }

        Ok((credit_limit > 0.0, credit_limit))
    }
}

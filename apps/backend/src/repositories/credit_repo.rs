use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CreditAccountFilterDto, CreditPlayerRow, CreditSettlement, CreditSettlementDetail,
    CreditSettlementFilterDto, CreditSettlementItemRow, CreditSettlementListRow, OutstandingTxnRow,
    SettleItemDto,
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

#[derive(sqlx::FromRow)]
struct CreditSettlementListRowWithTotal {
    id: Uuid,
    player_id: Uuid,
    player_username: String,
    shift_id: Uuid,
    settled_by: Uuid,
    settled_by_username: String,
    amount: f64,
    payment_method: String,
    cash_amount: Option<f64>,
    online_amount: Option<f64>,
    notes: Option<String>,
    settled_at: DateTime<Utc>,
    item_count: i64,
    total_count: i64,
}

impl From<CreditSettlementListRowWithTotal> for CreditSettlementListRow {
    fn from(row: CreditSettlementListRowWithTotal) -> Self {
        Self {
            id: row.id,
            player_id: row.player_id,
            player_username: row.player_username,
            shift_id: row.shift_id,
            settled_by: row.settled_by,
            settled_by_username: row.settled_by_username,
            amount: row.amount,
            payment_method: row.payment_method,
            cash_amount: row.cash_amount,
            online_amount: row.online_amount,
            notes: row.notes,
            settled_at: row.settled_at,
            item_count: row.item_count,
        }
    }
}

#[derive(sqlx::FromRow)]
struct CreditSettlementHeaderRow {
    id: Uuid,
    player_id: Uuid,
    player_username: String,
    shift_id: Uuid,
    settled_by: Uuid,
    settled_by_username: String,
    amount: f64,
    payment_method: String,
    cash_amount: Option<f64>,
    online_amount: Option<f64>,
    notes: Option<String>,
    settled_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

fn parse_date_start(value: Option<&str>) -> Option<DateTime<Utc>> {
    value.and_then(|s| {
        DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|d| d.with_timezone(&Utc))
            .or_else(|| {
                DateTime::parse_from_rfc3339(&format!("{s}T00:00:00+05:30"))
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            })
            .or_else(|| {
                DateTime::parse_from_rfc3339(&format!("{s}T00:00:00Z"))
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            })
    })
}

fn parse_date_end(value: Option<&str>) -> Option<DateTime<Utc>> {
    value.and_then(|s| {
        DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|d| d.with_timezone(&Utc))
            .or_else(|| {
                DateTime::parse_from_rfc3339(&format!("{s}T23:59:59+05:30"))
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            })
            .or_else(|| {
                DateTime::parse_from_rfc3339(&format!("{s}T23:59:59Z"))
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            })
    })
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

    pub async fn list_settlements(
        &self,
        filters: &CreditSettlementFilterDto,
    ) -> Result<PaginationResult<CreditSettlementListRow>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"
            SELECT cs.id,
                   cs."playerId" as player_id,
                   player.username as player_username,
                   cs."shiftId" as shift_id,
                   cs."settledBy" as settled_by,
                   settled.username as settled_by_username,
                   cs.amount::float8 as amount,
                   cs."paymentMethod" as payment_method,
                   cs."cashAmount"::float8 as cash_amount,
                   cs."onlineAmount"::float8 as online_amount,
                   cs.notes,
                   cs."settledAt" as settled_at,
                   (
                       SELECT COUNT(*)::bigint
                       FROM credit_settlement_items csi
                       WHERE csi."settlementId" = cs.id
                   ) as item_count,
                   COUNT(*) OVER() as total_count
            FROM credit_settlements cs
            INNER JOIN users player ON player.id = cs."playerId"
            INNER JOIN users settled ON settled.id = cs."settledBy"
            WHERE cs."deletedAt" IS NULL
            "#,
        );

        if let Some(player_id) = filters.player_id {
            builder.push(" AND cs.\"playerId\" = ");
            builder.push_bind(player_id);
        }

        if let Some(payment_method) = filters
            .payment_method
            .as_ref()
            .filter(|s| !s.is_empty())
        {
            builder.push(" AND cs.\"paymentMethod\" = ");
            builder.push_bind(payment_method);
        }

        if let Some(start) = parse_date_start(filters.start_date.as_deref()) {
            builder.push(" AND cs.\"settledAt\" >= ");
            builder.push_bind(start);
        }

        if let Some(end) = parse_date_end(filters.end_date.as_deref()) {
            builder.push(" AND cs.\"settledAt\" <= ");
            builder.push_bind(end);
        }

        if let Some(search) = filters.search.as_ref().filter(|s| !s.is_empty()) {
            let pattern = format!("%{search}%");
            builder.push(" AND (player.username ILIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR player.\"firstName\" ILIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR player.\"lastName\" ILIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR player.\"phoneNumber\" ILIKE ");
            builder.push_bind(pattern);
            builder.push(")");
        }

        let sort_by = filters.sort_by.as_deref().unwrap_or("settledAt");
        let sort_col = match sort_by {
            "amount" => "cs.amount",
            "playerUsername" => "player.username",
            "paymentMethod" => "cs.\"paymentMethod\"",
            _ => "cs.\"settledAt\"",
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
            .build_query_as::<CreditSettlementListRowWithTotal>()
            .fetch_all(&self.pool)
            .await?;

        let total = rows.first().map(|r| r.total_count).unwrap_or(0);
        let items: Vec<CreditSettlementListRow> = rows.into_iter().map(Into::into).collect();

        Ok(PaginationResult::new(items, total, page, limit))
    }

    pub async fn get_settlement_by_id(
        &self,
        id: Uuid,
    ) -> Result<CreditSettlementDetail, AppError> {
        let header = sqlx::query_as::<_, CreditSettlementHeaderRow>(
            r#"
            SELECT cs.id,
                   cs."playerId" as player_id,
                   player.username as player_username,
                   cs."shiftId" as shift_id,
                   cs."settledBy" as settled_by,
                   settled.username as settled_by_username,
                   cs.amount::float8 as amount,
                   cs."paymentMethod" as payment_method,
                   cs."cashAmount"::float8 as cash_amount,
                   cs."onlineAmount"::float8 as online_amount,
                   cs.notes,
                   cs."settledAt" as settled_at,
                   cs."createdAt" as created_at,
                   cs."updatedAt" as updated_at
            FROM credit_settlements cs
            INNER JOIN users player ON player.id = cs."playerId"
            INNER JOIN users settled ON settled.id = cs."settledBy"
            WHERE cs.id = $1 AND cs."deletedAt" IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Credit settlement {id} not found")))?;

        let items = sqlx::query_as::<_, CreditSettlementItemRow>(
            r#"
            SELECT t.id as transaction_id,
                   t."transactionType"::text as transaction_type,
                   t."transactionDate" as transaction_date,
                   t.amount::float8 as original_amount,
                   csi."amountApplied"::float8 as amount_applied,
                   GREATEST(t.amount - t."paidAmount", 0)::float8 as remaining_after
            FROM credit_settlement_items csi
            INNER JOIN transactions t ON t.id = csi."transactionId"
            WHERE csi."settlementId" = $1
            ORDER BY t."transactionDate" ASC
            "#,
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;

        Ok(CreditSettlementDetail {
            id: header.id,
            player_id: header.player_id,
            player_username: header.player_username,
            shift_id: header.shift_id,
            settled_by: header.settled_by,
            settled_by_username: header.settled_by_username,
            amount: header.amount,
            payment_method: header.payment_method,
            cash_amount: header.cash_amount,
            online_amount: header.online_amount,
            notes: header.notes,
            settled_at: header.settled_at,
            created_at: header.created_at,
            updated_at: header.updated_at,
            items,
        })
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

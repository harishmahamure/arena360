use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::PlayerPlanLedger;

pub struct LedgerRepository {
    pool: PgPool,
}

impl LedgerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn append(
        &self,
        balance_id: Uuid,
        player_id: Uuid,
        delta_minutes: i32,
        reason: &str,
        transaction_id: Option<Uuid>,
        session_id: Option<Uuid>,
        balance_after: i32,
        expiry_after: DateTime<Utc>,
        actor_id: Option<Uuid>,
    ) -> Result<PlayerPlanLedger, AppError> {
        let entry = sqlx::query_as::<_, PlayerPlanLedger>(
            r#"INSERT INTO player_plan_ledger (
                   "balanceId", "playerId", "deltaMinutes", reason,
                   "transactionId", "sessionId", "balanceAfter", "expiryAfter",
                   "createdBy"
               )
               VALUES ($1, $2, $3, $4::ledger_reason, $5, $6, $7, $8, $9)
               RETURNING id, "balanceId" as balance_id, "playerId" as player_id,
                   "deltaMinutes" as delta_minutes, reason::text as reason,
                   "transactionId" as transaction_id, "sessionId" as session_id,
                   "balanceAfter" as balance_after, "expiryAfter" as expiry_after,
                   "createdAt" as created_at, "createdBy" as created_by"#,
        )
        .bind(balance_id)
        .bind(player_id)
        .bind(delta_minutes)
        .bind(reason)
        .bind(transaction_id)
        .bind(session_id)
        .bind(balance_after)
        .bind(expiry_after)
        .bind(actor_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(entry)
    }

    pub async fn list_by_balance(
        &self,
        balance_id: Uuid,
    ) -> Result<Vec<PlayerPlanLedger>, AppError> {
        let entries = sqlx::query_as::<_, PlayerPlanLedger>(
            r#"SELECT id, "balanceId" as balance_id, "playerId" as player_id,
                   "deltaMinutes" as delta_minutes, reason::text as reason,
                   "transactionId" as transaction_id, "sessionId" as session_id,
                   "balanceAfter" as balance_after, "expiryAfter" as expiry_after,
                   "createdAt" as created_at, "createdBy" as created_by
               FROM player_plan_ledger
               WHERE "balanceId" = $1
               ORDER BY "createdAt" DESC"#,
        )
        .bind(balance_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(entries)
    }

    pub async fn find_grant_delta_for_balance(
        &self,
        balance_id: Uuid,
    ) -> Result<Option<i32>, AppError> {
        let row = sqlx::query_as::<_, (i32,)>(
            r#"SELECT "deltaMinutes" FROM player_plan_ledger
               WHERE "balanceId" = $1
                 AND reason::text IN ('staff_allowance_grant', 'staff_allowance_renewal')
               ORDER BY "createdAt" ASC
               LIMIT 1"#,
        )
        .bind(balance_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(delta,)| delta))
    }
}

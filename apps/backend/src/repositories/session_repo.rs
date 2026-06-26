use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CreateSessionDto, SessionFilterDto, UsageSession, UsageSessionResponse, UsageSessionRow,
};

#[derive(Debug, Clone)]
pub struct PlayerOpenSession {
    pub session_id: Uuid,
    pub device_id: Uuid,
    pub device_name: String,
    pub start_time: DateTime<Utc>,
    pub balance_id: Uuid,
    pub remaining_minutes: i32,
}

pub struct SessionRepository {
    pool: PgPool,
}

impl SessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id,
               "balanceId" as balance_id,
               "deviceId" as device_id,
               "shiftId" as shift_id,
               "startTime" as start_time,
               "endTime" as end_time,
               "durationMinutes" as duration_minutes,
               "timeCreditsConsumed" as time_credits_consumed,
               "walletMinutesAtStart" as wallet_minutes_at_start,
               "sourcePlanIdAtStart" as source_plan_id_at_start,
               "createdBy" as created_by,
               "updatedBy" as updated_by,
               "createdAt" as created_at,
               "updatedAt" as updated_at,
               "deletedAt" as deleted_at
        FROM usage_sessions
    "#;

    const ENRICHED_SELECT: &'static str = r#"
        SELECT s.id, s."balanceId" as balance_id, s."deviceId" as device_id,
               s."shiftId" as shift_id, s."startTime" as start_time, s."endTime" as end_time,
               s."durationMinutes" as duration_minutes, s."timeCreditsConsumed" as time_credits_consumed,
               s."walletMinutesAtStart" as wallet_minutes_at_start,
               s."sourcePlanIdAtStart" as source_plan_id_at_start,
               s."createdBy" as created_by, s."updatedBy" as updated_by,
               s."createdAt" as created_at, s."updatedAt" as updated_at, s."deletedAt" as deleted_at,
               b."playerId" as bal_player_id, b.kind::text as bal_kind,
               b."remainingMinutes" as bal_remaining_minutes, b.status::text as bal_status,
               b."sourcePlanId" as bal_source_plan_id,
               b."deductionProfile" as bal_deduction_profile,
               b."expiryDate" as bal_expiry_date,
               u.username as player_username, u."firstName" as player_first_name,
               u."lastName" as player_last_name,
               p.name as plan_name, p."planType"::text as plan_type, p."timeCredits" as plan_time_credits,
               d.name as device_name, d."deviceType"::text as device_type,
               d.location as device_location, d.status::text as device_status,
               p_start.name as plan_start_name, p_start."planType"::text as plan_start_type,
               p_start."timeCredits" as plan_start_time_credits
    "#;

    const ENRICHED_FROM: &'static str = r#"
        FROM usage_sessions s
        LEFT JOIN player_plan_balances b ON b.id = s."balanceId" AND b."deletedAt" IS NULL
        LEFT JOIN users u ON u.id = b."playerId" AND u."deletedAt" IS NULL
        LEFT JOIN plans p ON p.id = b."sourcePlanId" AND p."deletedAt" IS NULL
        LEFT JOIN plans p_start ON p_start.id = s."sourcePlanIdAtStart" AND p_start."deletedAt" IS NULL
        LEFT JOIN devices d ON d.id = s."deviceId" AND d."deletedAt" IS NULL
    "#;

    const SESSION_RETURNING: &'static str = r#"
        RETURNING id,
                  "balanceId" as balance_id,
                  "deviceId" as device_id,
                  "shiftId" as shift_id,
                  "startTime" as start_time,
                  "endTime" as end_time,
                  "durationMinutes" as duration_minutes,
                  "timeCreditsConsumed" as time_credits_consumed,
                  "walletMinutesAtStart" as wallet_minutes_at_start,
                  "sourcePlanIdAtStart" as source_plan_id_at_start,
                  "createdBy" as created_by,
                  "updatedBy" as updated_by,
                  "createdAt" as created_at,
                  "updatedAt" as updated_at,
                  "deletedAt" as deleted_at
    "#;

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<UsageSession>, AppError> {
        let query = format!("{} WHERE id = $1 AND \"deletedAt\" IS NULL", Self::SELECT);
        let session = sqlx::query_as::<_, UsageSession>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(session)
    }

    pub async fn find_enriched_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<UsageSessionResponse>, AppError> {
        let row = sqlx::query_as::<_, UsageSessionRow>(
            &format!(
                r#"{ENRICHED_SELECT}
            {ENRICHED_FROM}
            WHERE s.id = $1 AND s."deletedAt" IS NULL
            "#,
                ENRICHED_SELECT = Self::ENRICHED_SELECT,
                ENRICHED_FROM = Self::ENRICHED_FROM,
            ),
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.into_response()))
    }

    pub async fn find_active_by_balance(
        &self,
        balance_id: Uuid,
    ) -> Result<Vec<UsageSession>, AppError> {
        let query = format!(
            "{} WHERE \"balanceId\" = $1 AND \"endTime\" IS NULL AND \"deletedAt\" IS NULL",
            Self::SELECT
        );
        let sessions = sqlx::query_as::<_, UsageSession>(&query)
            .bind(balance_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(sessions)
    }

    pub async fn find_open_session_for_device(
        &self,
        device_id: Uuid,
    ) -> Result<Option<UsageSession>, AppError> {
        let query = format!(
            r#"{} WHERE "deviceId" = $1 AND "endTime" IS NULL AND "deletedAt" IS NULL
            ORDER BY "startTime" DESC LIMIT 1"#,
            Self::SELECT
        );
        let session = sqlx::query_as::<_, UsageSession>(&query)
            .bind(device_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(session)
    }

    pub async fn find_open_session_for_player(
        &self,
        player_id: Uuid,
    ) -> Result<Option<PlayerOpenSession>, AppError> {
        let row = sqlx::query_as::<_, (Uuid, Uuid, String, DateTime<Utc>, Uuid, i32)>(
            r#"
            SELECT s.id, s."deviceId", COALESCE(d.name, 'Unknown'), s."startTime",
                   s."balanceId", COALESCE(b."remainingMinutes", 0)
            FROM usage_sessions s
            INNER JOIN player_plan_balances b ON b.id = s."balanceId" AND b."deletedAt" IS NULL
            LEFT JOIN devices d ON d.id = s."deviceId" AND d."deletedAt" IS NULL
            WHERE b."playerId" = $1
              AND s."endTime" IS NULL
              AND s."deletedAt" IS NULL
            ORDER BY s."startTime" DESC
            LIMIT 1
            "#,
        )
        .bind(player_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(
            |(session_id, device_id, device_name, start_time, balance_id, remaining_minutes)| {
                PlayerOpenSession {
                    session_id,
                    device_id,
                    device_name,
                    start_time,
                    balance_id,
                    remaining_minutes,
                }
            },
        ))
    }

    /// Find or idempotently create the per-venue system kiosk shift (ADR-0017
    /// D10). The well-known marker is `notes = 'KIOSK_SYSTEM'`. A shift row
    /// requires a `userId`, so we attribute it to the oldest admin user.
    /// Returns `None` when no admin user exists yet (session then has no shift).
    pub async fn find_or_create_system_kiosk_shift(&self) -> Result<Option<Uuid>, AppError> {
        if let Some((id,)) = sqlx::query_as::<_, (Uuid,)>(
            r#"SELECT id FROM shifts WHERE notes = 'KIOSK_SYSTEM' AND status = 'active' LIMIT 1"#,
        )
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok(Some(id));
        }

        let created = sqlx::query_as::<_, (Uuid,)>(
            r#"
            INSERT INTO shifts (id, "userId", "clockIn", notes, status, "createdBy", "updatedBy", "createdAt", "updatedAt")
            SELECT gen_random_uuid(), u.id, NOW(), 'KIOSK_SYSTEM', 'active', u.id, u.id, NOW(), NOW()
            FROM (
                SELECT id FROM users
                WHERE role = 'admin' AND "deletedAt" IS NULL
                ORDER BY "createdAt" ASC
                LIMIT 1
            ) u
            RETURNING id
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(created.map(|(id,)| id))
    }

    pub async fn list(
        &self,
        filters: &SessionFilterDto,
    ) -> Result<PaginationResult<UsageSessionResponse>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            &format!(
                r#"{ENRICHED_SELECT}
               {ENRICHED_FROM}
               WHERE s."deletedAt" IS NULL"#,
                ENRICHED_SELECT = Self::ENRICHED_SELECT,
                ENRICHED_FROM = Self::ENRICHED_FROM,
            ),
        );

        Self::apply_filters(&mut builder, filters, "s");

        let sort_by = filters.sort_by.as_deref().unwrap_or("startTime");
        let sort_col = match sort_by {
            "endTime" => "s.\"endTime\"",
            "durationMinutes" => "s.\"durationMinutes\"",
            "createdAt" => "s.\"createdAt\"",
            _ => "s.\"startTime\"",
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
            .build_query_as::<UsageSessionRow>()
            .fetch_all(&self.pool)
            .await?;

        let data = rows.into_iter().map(|r| r.into_response()).collect();

        let mut count_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT COUNT(*) FROM usage_sessions s WHERE s.\"deletedAt\" IS NULL",
        );
        Self::apply_filters(&mut count_builder, filters, "s");

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(data, total.0, page, limit))
    }

    fn apply_filters(
        builder: &mut QueryBuilder<Postgres>,
        filters: &SessionFilterDto,
        prefix: &str,
    ) {
        let col = |name: &str| format!("{prefix}.\"{name}\"");
        if let Some(balance_id) = filters.balance_id {
            builder.push(format!(" AND {} = ", col("balanceId")));
            builder.push_bind(balance_id);
        }
        if let Some(device_id) = filters.device_id {
            builder.push(format!(" AND {} = ", col("deviceId")));
            builder.push_bind(device_id);
        }
        if let Some(shift_id) = filters.shift_id {
            builder.push(format!(" AND {} = ", col("shiftId")));
            builder.push_bind(shift_id);
        }
        if let Some(player_id) = filters.player_id {
            builder.push(format!(
                " AND EXISTS (SELECT 1 FROM player_plan_balances ppb WHERE ppb.id = {} AND ppb.\"playerId\" = ",
                col("balanceId")
            ));
            builder.push_bind(player_id);
            builder.push(" AND ppb.\"deletedAt\" IS NULL)");
        }
        if let Some(is_active) = filters.is_active {
            if is_active == 1 {
                builder.push(format!(" AND {} IS NULL", col("endTime")));
            } else {
                builder.push(format!(" AND {} IS NOT NULL", col("endTime")));
            }
        }
        if let Some(from) = filters.start_time_from {
            builder.push(format!(" AND {} >= ", col("startTime")));
            builder.push_bind(from);
        }
        if let Some(to) = filters.start_time_to {
            builder.push(format!(" AND {} <= ", col("startTime")));
            builder.push_bind(to);
        }
    }

    pub async fn create(
        &self,
        dto: &CreateSessionDto,
        start_time: DateTime<Utc>,
        actor_id: Option<Uuid>,
        wallet_minutes_at_start: i32,
        source_plan_id_at_start: Option<Uuid>,
    ) -> Result<UsageSession, AppError> {
        let query = format!(
            r#"
            INSERT INTO usage_sessions (
                id, "balanceId", "deviceId", "shiftId", "startTime",
                "walletMinutesAtStart", "sourcePlanIdAtStart",
                "createdBy", "updatedBy", "createdAt", "updatedAt"
            )
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $7, NOW(), NOW())
            {returning}
            "#,
            returning = Self::SESSION_RETURNING,
        );
        let session = sqlx::query_as::<_, UsageSession>(&query)
        .bind(dto.balance_id)
        .bind(dto.device_id)
        .bind(dto.shift_id)
        .bind(start_time)
        .bind(wallet_minutes_at_start)
        .bind(source_plan_id_at_start)
        .bind(actor_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(session)
    }

    pub async fn end(
        &self,
        id: Uuid,
        end_time: DateTime<Utc>,
        duration_minutes: i32,
        time_credits_consumed: Option<i32>,
        actor_id: Option<Uuid>,
    ) -> Result<UsageSession, AppError> {
        let session = sqlx::query_as::<_, UsageSession>(
            &format!(
                r#"
            UPDATE usage_sessions SET
                "endTime" = $2,
                "durationMinutes" = $3,
                "timeCreditsConsumed" = $4,
                "updatedBy" = COALESCE($5, "updatedBy"),
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL AND "endTime" IS NULL
            {returning}
            "#,
                returning = Self::SESSION_RETURNING,
            ),
        )
        .bind(id)
        .bind(end_time)
        .bind(duration_minutes)
        .bind(time_credits_consumed)
        .bind(actor_id)
        .fetch_optional(&self.pool)
        .await?;

        session.ok_or_else(|| AppError::NotFound(format!("Active session with ID {id} not found")))
    }

    pub async fn update_time_credits_consumed(
        &self,
        id: Uuid,
        time_credits_consumed: i32,
    ) -> Result<UsageSession, AppError> {
        let session = sqlx::query_as::<_, UsageSession>(
            &format!(
                r#"
            UPDATE usage_sessions SET
                "timeCreditsConsumed" = $2,
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL AND "endTime" IS NULL
            {returning}
            "#,
                returning = Self::SESSION_RETURNING,
            ),
        )
        .bind(id)
        .bind(time_credits_consumed)
        .fetch_optional(&self.pool)
        .await?;

        session.ok_or_else(|| AppError::NotFound(format!("Active session with ID {id} not found")))
    }
}

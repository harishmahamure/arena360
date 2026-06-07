use chrono::{DateTime, NaiveTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{BalanceFilterDto, BalanceRow, PlayerPlanBalance, PlayerPlanBalanceResponse};

pub struct BalanceRepository {
    pool: PgPool,
}

impl BalanceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id, "playerId" as player_id,
               "deviceType"::text as device_type, "deviceSubType"::text as device_sub_type,
               kind::text as kind, "remainingMinutes" as remaining_minutes,
               "expiryDate" as expiry_date, "windowStart" as window_start,
               "windowEnd" as window_end, status::text as status,
               "sourcePlanId" as source_plan_id,
               "allowedDays" as allowed_days, "allowedMonths" as allowed_months,
               "deductionProfile" as deduction_profile,
               "createdBy" as created_by, "updatedBy" as updated_by,
               "createdAt" as created_at, "updatedAt" as updated_at,
               "deletedAt" as deleted_at
        FROM player_plan_balances
    "#;

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<PlayerPlanBalance>, AppError> {
        let query = format!("{} WHERE id = $1 AND \"deletedAt\" IS NULL", Self::SELECT);
        let row = sqlx::query_as::<_, PlayerPlanBalance>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn find_enriched_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<PlayerPlanBalanceResponse>, AppError> {
        let row = sqlx::query_as::<_, BalanceRow>(
            r#"SELECT b.id, b."playerId" as player_id,
                   b."deviceType"::text as device_type, b."deviceSubType"::text as device_sub_type,
                   b.kind::text as kind, b."remainingMinutes" as remaining_minutes,
                   b."expiryDate" as expiry_date, b."windowStart" as window_start,
                   b."windowEnd" as window_end, b.status::text as status,
                   b."sourcePlanId" as source_plan_id,
                   b."allowedDays" as allowed_days, b."allowedMonths" as allowed_months,
                   b."deductionProfile" as deduction_profile,
                   b."createdBy" as created_by, b."updatedBy" as updated_by,
                   b."createdAt" as created_at, b."updatedAt" as updated_at,
                   b."deletedAt" as deleted_at,
                   u.username as player_username, u."firstName" as player_first_name,
                   u."lastName" as player_last_name,
                   p.name as plan_name, p."planType"::text as plan_type,
                   p.price::float8 as plan_price, p."timeCredits" as plan_time_credits
            FROM player_plan_balances b
            LEFT JOIN users u ON u.id = b."playerId" AND u."deletedAt" IS NULL
            LEFT JOIN plans p ON p.id = b."sourcePlanId" AND p."deletedAt" IS NULL
            WHERE b.id = $1 AND b."deletedAt" IS NULL"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.into_response()))
    }

    pub async fn find_active(
        &self,
        player_id: Uuid,
        device_type: Option<&str>,
        device_sub_type: Option<&str>,
        kind: &str,
    ) -> Result<Option<PlayerPlanBalance>, AppError> {
        let row = sqlx::query_as::<_, PlayerPlanBalance>(
            r#"SELECT id, "playerId" as player_id,
                   "deviceType"::text as device_type, "deviceSubType"::text as device_sub_type,
                   kind::text as kind, "remainingMinutes" as remaining_minutes,
                   "expiryDate" as expiry_date, "windowStart" as window_start,
                   "windowEnd" as window_end, status::text as status,
                   "sourcePlanId" as source_plan_id,
                   "allowedDays" as allowed_days, "allowedMonths" as allowed_months,
                   "deductionProfile" as deduction_profile,
                   "createdBy" as created_by, "updatedBy" as updated_by,
                   "createdAt" as created_at, "updatedAt" as updated_at,
                   "deletedAt" as deleted_at
            FROM player_plan_balances
            WHERE "playerId" = $1
              AND COALESCE("deviceType"::text, '__null__') = COALESCE($2, '__null__')
              AND COALESCE("deviceSubType"::text, '__null__') = COALESCE($3, '__null__')
              AND kind::text = $4
              AND status::text = 'active'
              AND "deletedAt" IS NULL
              AND "expiryDate" > NOW()
              AND "remainingMinutes" > 0
            ORDER BY "createdAt" DESC
            LIMIT 1"#,
        )
        .bind(player_id)
        .bind(device_type)
        .bind(device_sub_type)
        .bind(kind)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn find_existing_for_scope(
        &self,
        player_id: Uuid,
        device_type: Option<&str>,
        device_sub_type: Option<&str>,
        kind: &str,
    ) -> Result<Option<PlayerPlanBalance>, AppError> {
        let row = sqlx::query_as::<_, PlayerPlanBalance>(
            r#"SELECT id, "playerId" as player_id,
                   "deviceType"::text as device_type, "deviceSubType"::text as device_sub_type,
                   kind::text as kind, "remainingMinutes" as remaining_minutes,
                   "expiryDate" as expiry_date, "windowStart" as window_start,
                   "windowEnd" as window_end, status::text as status,
                   "sourcePlanId" as source_plan_id,
                   "allowedDays" as allowed_days, "allowedMonths" as allowed_months,
                   "deductionProfile" as deduction_profile,
                   "createdBy" as created_by, "updatedBy" as updated_by,
                   "createdAt" as created_at, "updatedAt" as updated_at,
                   "deletedAt" as deleted_at
            FROM player_plan_balances
            WHERE "playerId" = $1
              AND COALESCE("deviceType"::text, '__null__') = COALESCE($2, '__null__')
              AND COALESCE("deviceSubType"::text, '__null__') = COALESCE($3, '__null__')
              AND kind::text = $4
              AND "deletedAt" IS NULL
            ORDER BY "createdAt" DESC
            LIMIT 1"#,
        )
        .bind(player_id)
        .bind(device_type)
        .bind(device_sub_type)
        .bind(kind)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        player_id: Uuid,
        device_type: Option<&str>,
        device_sub_type: Option<&str>,
        kind: &str,
        remaining_minutes: i32,
        expiry_date: DateTime<Utc>,
        window_start: Option<NaiveTime>,
        window_end: Option<NaiveTime>,
        source_plan_id: Uuid,
        allowed_days: Option<&Value>,
        allowed_months: Option<&Value>,
        deduction_profile: Option<&Value>,
        actor_id: Option<Uuid>,
    ) -> Result<PlayerPlanBalance, AppError> {
        let balance = sqlx::query_as::<_, PlayerPlanBalance>(
            r#"INSERT INTO player_plan_balances (
                   "playerId", "deviceType", "deviceSubType", kind,
                   "remainingMinutes", "expiryDate", "windowStart", "windowEnd",
                   status, "sourcePlanId", "allowedDays", "allowedMonths",
                   "deductionProfile", "createdBy", "updatedBy"
               )
               VALUES ($1, $2::plans_devicetype_enum, $3::plans_devicesubtype_enum,
                       $4::plan_kind, $5, $6, $7, $8,
                       'active'::balance_status, $9, $10, $11, $12,
                       $13, $13)
               RETURNING id, "playerId" as player_id,
                   "deviceType"::text as device_type, "deviceSubType"::text as device_sub_type,
                   kind::text as kind, "remainingMinutes" as remaining_minutes,
                   "expiryDate" as expiry_date, "windowStart" as window_start,
                   "windowEnd" as window_end, status::text as status,
                   "sourcePlanId" as source_plan_id,
                   "allowedDays" as allowed_days, "allowedMonths" as allowed_months,
                   "deductionProfile" as deduction_profile,
                   "createdBy" as created_by, "updatedBy" as updated_by,
                   "createdAt" as created_at, "updatedAt" as updated_at,
                   "deletedAt" as deleted_at"#,
        )
        .bind(player_id)
        .bind(device_type)
        .bind(device_sub_type)
        .bind(kind)
        .bind(remaining_minutes)
        .bind(expiry_date)
        .bind(window_start)
        .bind(window_end)
        .bind(source_plan_id)
        .bind(allowed_days)
        .bind(allowed_months)
        .bind(deduction_profile)
        .bind(actor_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(balance)
    }

    pub async fn recharge(
        &self,
        id: Uuid,
        minutes: i32,
        new_expiry: DateTime<Utc>,
        source_plan_id: Uuid,
        deduction_profile: Option<&Value>,
        actor_id: Option<Uuid>,
        accumulate: bool,
    ) -> Result<PlayerPlanBalance, AppError> {
        let minutes_sql = if accumulate {
            r#""remainingMinutes" = "remainingMinutes" + $2"#
        } else {
            r#""remainingMinutes" = $2"#
        };
        let query = format!(
            r#"UPDATE player_plan_balances SET
                   {minutes_sql},
                   "expiryDate" = $3,
                   "sourcePlanId" = $4,
                   "deductionProfile" = COALESCE($6, "deductionProfile"),
                   status = 'active'::balance_status,
                   "updatedBy" = COALESCE($5, "updatedBy"),
                   "updatedAt" = NOW()
               WHERE id = $1 AND "deletedAt" IS NULL
               RETURNING id, "playerId" as player_id,
                   "deviceType"::text as device_type, "deviceSubType"::text as device_sub_type,
                   kind::text as kind, "remainingMinutes" as remaining_minutes,
                   "expiryDate" as expiry_date, "windowStart" as window_start,
                   "windowEnd" as window_end, status::text as status,
                   "sourcePlanId" as source_plan_id,
                   "allowedDays" as allowed_days, "allowedMonths" as allowed_months,
                   "deductionProfile" as deduction_profile,
                   "createdBy" as created_by, "updatedBy" as updated_by,
                   "createdAt" as created_at, "updatedAt" as updated_at,
                   "deletedAt" as deleted_at"#
        );
        let balance = sqlx::query_as::<_, PlayerPlanBalance>(&query)
        .bind(id)
        .bind(minutes)
        .bind(new_expiry)
        .bind(source_plan_id)
        .bind(actor_id)
        .bind(deduction_profile)
        .fetch_one(&self.pool)
        .await?;
        Ok(balance)
    }

    pub async fn deduct_minutes(
        &self,
        id: Uuid,
        minutes: i32,
    ) -> Result<PlayerPlanBalance, AppError> {
        let balance = sqlx::query_as::<_, PlayerPlanBalance>(
            r#"UPDATE player_plan_balances SET
                   "remainingMinutes" = GREATEST("remainingMinutes" - $2, 0),
                   status = CASE WHEN "remainingMinutes" - $2 <= 0
                                 THEN 'exhausted'::balance_status
                                 ELSE status END,
                   "updatedAt" = NOW()
               WHERE id = $1 AND "deletedAt" IS NULL
               RETURNING id, "playerId" as player_id,
                   "deviceType"::text as device_type, "deviceSubType"::text as device_sub_type,
                   kind::text as kind, "remainingMinutes" as remaining_minutes,
                   "expiryDate" as expiry_date, "windowStart" as window_start,
                   "windowEnd" as window_end, status::text as status,
                   "sourcePlanId" as source_plan_id,
                   "allowedDays" as allowed_days, "allowedMonths" as allowed_months,
                   "deductionProfile" as deduction_profile,
                   "createdBy" as created_by, "updatedBy" as updated_by,
                   "createdAt" as created_at, "updatedAt" as updated_at,
                   "deletedAt" as deleted_at"#,
        )
        .bind(id)
        .bind(minutes)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Balance with ID {id} not found")))?;
        Ok(balance)
    }

    pub async fn set_status(&self, id: Uuid, status: &str) -> Result<(), AppError> {
        sqlx::query(
            r#"UPDATE player_plan_balances SET
                   status = $2::balance_status, "updatedAt" = NOW()
               WHERE id = $1 AND "deletedAt" IS NULL"#,
        )
        .bind(id)
        .bind(status)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn expire_stale(&self) -> Result<u64, AppError> {
        let result = sqlx::query(
            r#"UPDATE player_plan_balances SET
                   status = 'expired'::balance_status, "updatedAt" = NOW()
               WHERE status::text = 'active'
                 AND "expiryDate" <= NOW()
                 AND "deletedAt" IS NULL"#,
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn list(
        &self,
        filters: &BalanceFilterDto,
    ) -> Result<PaginationResult<PlayerPlanBalanceResponse>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT b.id, b."playerId" as player_id,
               b."deviceType"::text as device_type, b."deviceSubType"::text as device_sub_type,
               b.kind::text as kind, b."remainingMinutes" as remaining_minutes,
               b."expiryDate" as expiry_date, b."windowStart" as window_start,
               b."windowEnd" as window_end, b.status::text as status,
               b."sourcePlanId" as source_plan_id,
               b."allowedDays" as allowed_days, b."allowedMonths" as allowed_months,
               b."deductionProfile" as deduction_profile,
               b."createdBy" as created_by, b."updatedBy" as updated_by,
               b."createdAt" as created_at, b."updatedAt" as updated_at,
               b."deletedAt" as deleted_at,
               u.username as player_username, u."firstName" as player_first_name,
               u."lastName" as player_last_name,
               p.name as plan_name, p."planType"::text as plan_type,
               p.price::float8 as plan_price, p."timeCredits" as plan_time_credits
               FROM player_plan_balances b
               LEFT JOIN users u ON u.id = b."playerId" AND u."deletedAt" IS NULL
               LEFT JOIN plans p ON p.id = b."sourcePlanId" AND p."deletedAt" IS NULL
               WHERE b."deletedAt" IS NULL"#,
        );

        Self::apply_filters(&mut builder, filters);

        let sort_by = filters.sort_by.as_deref().unwrap_or("createdAt");
        let sort_col = match sort_by {
            "expiryDate" => "b.\"expiryDate\"",
            "remainingMinutes" => "b.\"remainingMinutes\"",
            "status" => "b.status",
            _ => "b.\"createdAt\"",
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
            .build_query_as::<BalanceRow>()
            .fetch_all(&self.pool)
            .await?;

        let data = rows.into_iter().map(|r| r.into_response()).collect();

        let mut count_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT COUNT(*) FROM player_plan_balances b
               WHERE b."deletedAt" IS NULL"#,
        );
        Self::apply_filters(&mut count_builder, filters);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(data, total.0, page, limit))
    }

    fn apply_filters<'a>(builder: &mut QueryBuilder<'a, Postgres>, filters: &'a BalanceFilterDto) {
        if let Some(player_id) = filters.player_id {
            builder.push(" AND b.\"playerId\" = ");
            builder.push_bind(player_id);
        }
        if let Some(kind) = filters.kind.as_deref() {
            builder.push(" AND b.kind::text = ");
            builder.push_bind(kind);
        }
        if let Some(status) = filters.status.as_deref() {
            builder.push(" AND b.status::text = ");
            builder.push_bind(status);
        }
        if let Some(device_type) = filters.device_type.as_deref() {
            builder.push(" AND b.\"deviceType\"::text = ");
            builder.push_bind(device_type);
        }
        if let Some(device_sub_type) = filters.device_sub_type.as_deref() {
            builder.push(" AND b.\"deviceSubType\"::text = ");
            builder.push_bind(device_sub_type);
        }
        if filters.usable_only == Some(true) {
            builder.push(" AND b.\"expiryDate\" > NOW() AND b.\"remainingMinutes\" > 0");
        }
    }
}

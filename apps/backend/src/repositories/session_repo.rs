use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CreateSessionDto, SessionFilterDto, UsageSession, UsageSessionResponse, UsageSessionRow,
};

pub struct SessionRepository {
    pool: PgPool,
}

impl SessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id,
               "playerPlanId" as player_plan_id,
               "deviceId" as device_id,
               "shiftId" as shift_id,
               "startTime" as start_time,
               "endTime" as end_time,
               "durationMinutes" as duration_minutes,
               "timeCreditsConsumed" as time_credits_consumed,
               "createdBy" as created_by,
               "updatedBy" as updated_by,
               "createdAt" as created_at,
               "updatedAt" as updated_at,
               "deletedAt" as deleted_at
        FROM usage_sessions
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
            r#"
            SELECT s.id, s."playerPlanId" as player_plan_id, s."deviceId" as device_id,
                   s."shiftId" as shift_id, s."startTime" as start_time, s."endTime" as end_time,
                   s."durationMinutes" as duration_minutes, s."timeCreditsConsumed" as time_credits_consumed,
                   s."createdBy" as created_by, s."updatedBy" as updated_by,
                   s."createdAt" as created_at, s."updatedAt" as updated_at, s."deletedAt" as deleted_at,
                   pp."playerId" as pp_player_id, pp."planId" as pp_plan_id,
                   pp."remainingTimeCredits" as pp_remaining_time_credits, pp.status::text as pp_status,
                   u.username as player_username, u."firstName" as player_first_name,
                   u."lastName" as player_last_name,
                   p.name as plan_name, p."planType"::text as plan_type, p."timeCredits" as plan_time_credits,
                   d.name as device_name, d."deviceType"::text as device_type,
                   d.location as device_location, d.status::text as device_status
            FROM usage_sessions s
            LEFT JOIN player_plans pp ON pp.id = s."playerPlanId" AND pp."deletedAt" IS NULL
            LEFT JOIN users u ON u.id = pp."playerId" AND u."deletedAt" IS NULL
            LEFT JOIN plans p ON p.id = pp."planId" AND p."deletedAt" IS NULL
            LEFT JOIN devices d ON d.id = s."deviceId" AND d."deletedAt" IS NULL
            WHERE s.id = $1 AND s."deletedAt" IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.into_response()))
    }

    pub async fn find_active_by_player_plan(
        &self,
        player_plan_id: Uuid,
    ) -> Result<Vec<UsageSession>, AppError> {
        let query = format!(
            "{} WHERE \"playerPlanId\" = $1 AND \"endTime\" IS NULL AND \"deletedAt\" IS NULL",
            Self::SELECT
        );
        let sessions = sqlx::query_as::<_, UsageSession>(&query)
            .bind(player_plan_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(sessions)
    }

    pub async fn list(
        &self,
        filters: &SessionFilterDto,
    ) -> Result<PaginationResult<UsageSessionResponse>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT s.id, s."playerPlanId" as player_plan_id, s."deviceId" as device_id,
               s."shiftId" as shift_id, s."startTime" as start_time, s."endTime" as end_time,
               s."durationMinutes" as duration_minutes, s."timeCreditsConsumed" as time_credits_consumed,
               s."createdBy" as created_by, s."updatedBy" as updated_by,
               s."createdAt" as created_at, s."updatedAt" as updated_at, s."deletedAt" as deleted_at,
               pp."playerId" as pp_player_id, pp."planId" as pp_plan_id,
               pp."remainingTimeCredits" as pp_remaining_time_credits, pp.status::text as pp_status,
               u.username as player_username, u."firstName" as player_first_name,
               u."lastName" as player_last_name,
               p.name as plan_name, p."planType"::text as plan_type, p."timeCredits" as plan_time_credits,
               d.name as device_name, d."deviceType"::text as device_type,
               d.location as device_location, d.status::text as device_status
               FROM usage_sessions s
               LEFT JOIN player_plans pp ON pp.id = s."playerPlanId" AND pp."deletedAt" IS NULL
               LEFT JOIN users u ON u.id = pp."playerId" AND u."deletedAt" IS NULL
               LEFT JOIN plans p ON p.id = pp."planId" AND p."deletedAt" IS NULL
               LEFT JOIN devices d ON d.id = s."deviceId" AND d."deletedAt" IS NULL
               WHERE s."deletedAt" IS NULL"#,
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
        if let Some(player_plan_id) = filters.player_plan_id {
            builder.push(format!(" AND {} = ", col("playerPlanId")));
            builder.push_bind(player_plan_id);
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
                " AND {} IN (SELECT id FROM player_plans WHERE \"playerId\" = ",
                col("playerPlanId")
            ));
            builder.push_bind(player_id);
            builder.push(" AND \"deletedAt\" IS NULL)");
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
    ) -> Result<UsageSession, AppError> {
        let session = sqlx::query_as::<_, UsageSession>(
            r#"
            INSERT INTO usage_sessions (
                id, "playerPlanId", "deviceId", "shiftId", "startTime",
                "createdBy", "updatedBy", "createdAt", "updatedAt"
            )
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $5, NOW(), NOW())
            RETURNING id,
                      "playerPlanId" as player_plan_id,
                      "deviceId" as device_id,
                      "shiftId" as shift_id,
                      "startTime" as start_time,
                      "endTime" as end_time,
                      "durationMinutes" as duration_minutes,
                      "timeCreditsConsumed" as time_credits_consumed,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(dto.player_plan_id)
        .bind(dto.device_id)
        .bind(dto.shift_id)
        .bind(start_time)
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
            r#"
            UPDATE usage_sessions SET
                "endTime" = $2,
                "durationMinutes" = $3,
                "timeCreditsConsumed" = $4,
                "updatedBy" = COALESCE($5, "updatedBy"),
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL AND "endTime" IS NULL
            RETURNING id,
                      "playerPlanId" as player_plan_id,
                      "deviceId" as device_id,
                      "shiftId" as shift_id,
                      "startTime" as start_time,
                      "endTime" as end_time,
                      "durationMinutes" as duration_minutes,
                      "timeCreditsConsumed" as time_credits_consumed,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
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
}

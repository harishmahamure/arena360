use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{CreateSessionDto, SessionFilterDto, UsageSession};

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
               "gameId" as game_id,
               "startTime" as start_time,
               "endTime" as end_time,
               "durationMinutes" as duration_minutes,
               "timeCreditsConsumed" as time_credits_consumed,
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

    pub async fn list(&self, filters: &SessionFilterDto) -> Result<PaginationResult<UsageSession>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, \"playerPlanId\" as player_plan_id, \"deviceId\" as device_id, \
             \"gameId\" as game_id, \"startTime\" as start_time, \"endTime\" as end_time, \
             \"durationMinutes\" as duration_minutes, \"timeCreditsConsumed\" as time_credits_consumed, \
             \"createdAt\" as created_at, \"updatedAt\" as updated_at, \"deletedAt\" as deleted_at \
             FROM usage_sessions WHERE \"deletedAt\" IS NULL",
        );

        Self::apply_filters(&mut builder, filters);

        let sort_by = filters.sort_by.as_deref().unwrap_or("startTime");
        let sort_col = match sort_by {
            "endTime" => "\"endTime\"",
            "durationMinutes" => "\"durationMinutes\"",
            "createdAt" => "\"createdAt\"",
            _ => "\"startTime\"",
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

        let sessions = builder
            .build_query_as::<UsageSession>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM usage_sessions WHERE \"deletedAt\" IS NULL");
        Self::apply_filters(&mut count_builder, filters);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(sessions, total.0, page, limit))
    }

    fn apply_filters(builder: &mut QueryBuilder<Postgres>, filters: &SessionFilterDto) {
        if let Some(player_plan_id) = filters.player_plan_id {
            builder.push(" AND \"playerPlanId\" = ");
            builder.push_bind(player_plan_id);
        }
        if let Some(device_id) = filters.device_id {
            builder.push(" AND \"deviceId\" = ");
            builder.push_bind(device_id);
        }
        if let Some(game_id) = filters.game_id {
            builder.push(" AND \"gameId\" = ");
            builder.push_bind(game_id);
        }
        if let Some(player_id) = filters.player_id {
            builder.push(
                " AND \"playerPlanId\" IN (SELECT id FROM player_plans WHERE \"playerId\" = ",
            );
            builder.push_bind(player_id);
            builder.push(" AND \"deletedAt\" IS NULL)");
        }
        if let Some(is_active) = filters.is_active {
            if is_active == 1 {
                builder.push(" AND \"endTime\" IS NULL");
            } else {
                builder.push(" AND \"endTime\" IS NOT NULL");
            }
        }
        if let Some(from) = filters.start_time_from {
            builder.push(" AND \"startTime\" >= ");
            builder.push_bind(from);
        }
        if let Some(to) = filters.start_time_to {
            builder.push(" AND \"startTime\" <= ");
            builder.push_bind(to);
        }
    }

    pub async fn create(&self, dto: &CreateSessionDto, start_time: DateTime<Utc>) -> Result<UsageSession, AppError> {
        let session = sqlx::query_as::<_, UsageSession>(
            r#"
            INSERT INTO usage_sessions (
                id, "playerPlanId", "deviceId", "gameId", "startTime", "createdAt", "updatedAt"
            )
            VALUES (gen_random_uuid(), $1, $2, $3, $4, NOW(), NOW())
            RETURNING id,
                      "playerPlanId" as player_plan_id,
                      "deviceId" as device_id,
                      "gameId" as game_id,
                      "startTime" as start_time,
                      "endTime" as end_time,
                      "durationMinutes" as duration_minutes,
                      "timeCreditsConsumed" as time_credits_consumed,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(dto.player_plan_id)
        .bind(dto.device_id)
        .bind(dto.game_id)
        .bind(start_time)
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
    ) -> Result<UsageSession, AppError> {
        let session = sqlx::query_as::<_, UsageSession>(
            r#"
            UPDATE usage_sessions SET
                "endTime" = $2,
                "durationMinutes" = $3,
                "timeCreditsConsumed" = $4,
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL AND "endTime" IS NULL
            RETURNING id,
                      "playerPlanId" as player_plan_id,
                      "deviceId" as device_id,
                      "gameId" as game_id,
                      "startTime" as start_time,
                      "endTime" as end_time,
                      "durationMinutes" as duration_minutes,
                      "timeCreditsConsumed" as time_credits_consumed,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .bind(end_time)
        .bind(duration_minutes)
        .bind(time_credits_consumed)
        .fetch_optional(&self.pool)
        .await?;

        session.ok_or_else(|| AppError::NotFound(format!("Active session with ID {id} not found")))
    }
}

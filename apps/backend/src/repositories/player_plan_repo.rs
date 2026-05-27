use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    PlayerPlan, PlayerPlanCreateValues, PlayerPlanFilterDto, PlayerPlanResponse, PlayerPlanRow,
    PlayerPlanUpdateValues,
};

pub struct PlayerPlanRepository {
    pool: PgPool,
}

impl PlayerPlanRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id, "playerId" as player_id, "planId" as plan_id,
               "purchaseDate" as purchase_date, "activationDate" as activation_date,
               "expiryDate" as expiry_date, "remainingUsageCount" as remaining_usage_count,
               "remainingTimeCredits" as remaining_time_credits, status::text as status,
               "movedToPlanId" as moved_to_plan_id,
               "movedCreditsCount" as moved_credits_count,
               "createdBy" as created_by, "updatedBy" as updated_by,
               "createdAt" as created_at, "updatedAt" as updated_at,
               "deletedAt" as deleted_at
        FROM player_plans
    "#;

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<PlayerPlan>, AppError> {
        let query = format!("{} WHERE id = $1 AND \"deletedAt\" IS NULL", Self::SELECT);
        let player_plan = sqlx::query_as::<_, PlayerPlan>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(player_plan)
    }

    pub async fn find_enriched_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<PlayerPlanResponse>, AppError> {
        let query = r#"
            SELECT pp.id, pp."playerId" as player_id, pp."planId" as plan_id,
                   pp."purchaseDate" as purchase_date, pp."activationDate" as activation_date,
                   pp."expiryDate" as expiry_date, pp."remainingUsageCount" as remaining_usage_count,
                   pp."remainingTimeCredits" as remaining_time_credits, pp.status::text as status,
                   pp."movedToPlanId" as moved_to_plan_id,
                   pp."movedCreditsCount" as moved_credits_count,
                   pp."createdBy" as created_by, pp."updatedBy" as updated_by,
                   pp."createdAt" as created_at, pp."updatedAt" as updated_at,
                   pp."deletedAt" as deleted_at,
                   u.username as player_username, u."firstName" as player_first_name,
                   u."lastName" as player_last_name,
                   p.name as plan_name, p."planType"::text as plan_type,
                   p.price::float8 as plan_price, p."timeCredits" as plan_time_credits
            FROM player_plans pp
            LEFT JOIN users u ON u.id = pp."playerId" AND u."deletedAt" IS NULL
            LEFT JOIN plans p ON p.id = pp."planId" AND p."deletedAt" IS NULL
            WHERE pp.id = $1 AND pp."deletedAt" IS NULL
            "#
        .to_string();
        let row = sqlx::query_as::<_, PlayerPlanRow>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.into_response()))
    }

    pub async fn create(
        &self,
        values: &PlayerPlanCreateValues,
        actor_id: Option<Uuid>,
    ) -> Result<PlayerPlan, AppError> {
        let player_plan = sqlx::query_as::<_, PlayerPlan>(
            r#"INSERT INTO player_plans (
                   "playerId", "planId", "purchaseDate", "expiryDate",
                   "remainingUsageCount", "remainingTimeCredits", status,
                   "createdBy", "updatedBy"
               )
               VALUES ($1, $2, $3, $4, $5, $6, $7::player_plans_status_enum, $8, $8)
               RETURNING id, "playerId" as player_id, "planId" as plan_id,
                   "purchaseDate" as purchase_date, "activationDate" as activation_date,
                   "expiryDate" as expiry_date, "remainingUsageCount" as remaining_usage_count,
                   "remainingTimeCredits" as remaining_time_credits, status::text as status,
                   "movedToPlanId" as moved_to_plan_id,
                   "movedCreditsCount" as moved_credits_count,
                   "createdBy" as created_by, "updatedBy" as updated_by,
                   "createdAt" as created_at, "updatedAt" as updated_at,
                   "deletedAt" as deleted_at"#,
        )
        .bind(values.player_id)
        .bind(values.plan_id)
        .bind(values.purchase_date)
        .bind(values.expiry_date)
        .bind(values.remaining_usage_count)
        .bind(values.remaining_time_credits)
        .bind(&values.status)
        .bind(actor_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(player_plan)
    }

    pub async fn update(
        &self,
        id: Uuid,
        values: &PlayerPlanUpdateValues,
        actor_id: Option<Uuid>,
    ) -> Result<PlayerPlan, AppError> {
        let player_plan = sqlx::query_as::<_, PlayerPlan>(
            r#"UPDATE player_plans SET
                   status = COALESCE($2::player_plans_status_enum, status),
                   "remainingTimeCredits" = COALESCE($3, "remainingTimeCredits"),
                   "remainingUsageCount" = COALESCE($4, "remainingUsageCount"),
                   "activationDate" = COALESCE($5, "activationDate"),
                   "updatedBy" = COALESCE($6, "updatedBy"),
                   "updatedAt" = NOW()
               WHERE id = $1 AND "deletedAt" IS NULL
               RETURNING id, "playerId" as player_id, "planId" as plan_id,
                   "purchaseDate" as purchase_date, "activationDate" as activation_date,
                   "expiryDate" as expiry_date, "remainingUsageCount" as remaining_usage_count,
                   "remainingTimeCredits" as remaining_time_credits, status::text as status,
                   "movedToPlanId" as moved_to_plan_id,
                   "movedCreditsCount" as moved_credits_count,
                   "createdBy" as created_by, "updatedBy" as updated_by,
                   "createdAt" as created_at, "updatedAt" as updated_at,
                   "deletedAt" as deleted_at"#,
        )
        .bind(id)
        .bind(&values.status)
        .bind(values.remaining_time_credits)
        .bind(values.remaining_usage_count)
        .bind(values.activation_date)
        .bind(actor_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Player plan with ID {id} not found")))?;

        Ok(player_plan)
    }

    pub async fn find_expired_active(&self) -> Result<Vec<PlayerPlan>, AppError> {
        let query = format!(
            r#"{} WHERE status::text = 'active' AND "expiryDate" <= NOW() AND "deletedAt" IS NULL"#,
            Self::SELECT
        );
        let player_plans = sqlx::query_as::<_, PlayerPlan>(&query)
            .fetch_all(&self.pool)
            .await?;
        Ok(player_plans)
    }

    pub async fn list(
        &self,
        filters: &PlayerPlanFilterDto,
    ) -> Result<PaginationResult<PlayerPlanResponse>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let join_plan = filters.device_type.is_some() || filters.device_sub_type.is_some();

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT pp.id, pp."playerId" as player_id, pp."planId" as plan_id,
               pp."purchaseDate" as purchase_date, pp."activationDate" as activation_date,
               pp."expiryDate" as expiry_date, pp."remainingUsageCount" as remaining_usage_count,
               pp."remainingTimeCredits" as remaining_time_credits, pp.status::text as status,
               pp."movedToPlanId" as moved_to_plan_id,
               pp."movedCreditsCount" as moved_credits_count,
               pp."createdBy" as created_by, pp."updatedBy" as updated_by,
               pp."createdAt" as created_at, pp."updatedAt" as updated_at,
               pp."deletedAt" as deleted_at,
               u.username as player_username, u."firstName" as player_first_name,
               u."lastName" as player_last_name,
               p.name as plan_name, p."planType"::text as plan_type,
               p.price::float8 as plan_price, p."timeCredits" as plan_time_credits
               FROM player_plans pp
               LEFT JOIN users u ON u.id = pp."playerId" AND u."deletedAt" IS NULL
               LEFT JOIN plans p ON p.id = pp."planId" AND p."deletedAt" IS NULL"#,
        );

        builder.push(" WHERE pp.\"deletedAt\" IS NULL");

        Self::apply_filters(&mut builder, filters, join_plan);

        let sort_by = filters.sort_by.as_deref().unwrap_or("purchaseDate");
        let sort_col = match sort_by {
            "expiryDate" => "pp.\"expiryDate\"",
            "status" => "pp.status",
            "createdAt" => "pp.\"createdAt\"",
            _ => "pp.\"purchaseDate\"",
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

        let mut rows = builder
            .build_query_as::<PlayerPlanRow>()
            .fetch_all(&self.pool)
            .await?;

        Self::apply_post_filters_rows(&mut rows, filters);

        let data = rows.into_iter().map(|r| r.into_response()).collect();

        let mut count_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT COUNT(*) FROM player_plans pp
               LEFT JOIN plans p ON p.id = pp."planId" AND p."deletedAt" IS NULL
               WHERE pp."deletedAt" IS NULL"#,
        );
        Self::apply_filters(&mut count_builder, filters, join_plan);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(data, total.0, page, limit))
    }

    fn apply_filters<'a>(
        builder: &mut QueryBuilder<'a, Postgres>,
        filters: &'a PlayerPlanFilterDto,
        join_plan: bool,
    ) {
        if let Some(player_id) = filters.player_id {
            builder.push(" AND pp.\"playerId\" = ");
            builder.push_bind(player_id);
        }
        if let Some(plan_id) = filters.plan_id {
            builder.push(" AND pp.\"planId\" = ");
            builder.push_bind(plan_id);
        }
        if let Some(status) = filters.status.clone() {
            builder.push(" AND pp.status::text = ");
            builder.push_bind(status);
        }
        if let Some(from) = &filters.purchase_date_from {
            if let Ok(dt) = from.parse::<DateTime<Utc>>() {
                builder.push(" AND pp.\"purchaseDate\" >= ");
                builder.push_bind(dt);
            }
        }
        if let Some(to) = &filters.purchase_date_to {
            if let Ok(dt) = to.parse::<DateTime<Utc>>() {
                builder.push(" AND pp.\"purchaseDate\" <= ");
                builder.push_bind(dt);
            }
        }
        if let Some(from) = &filters.expiry_date_from {
            if let Ok(dt) = from.parse::<DateTime<Utc>>() {
                builder.push(" AND pp.\"expiryDate\" >= ");
                builder.push_bind(dt);
            }
        }
        if let Some(to) = &filters.expiry_date_to {
            if let Ok(dt) = to.parse::<DateTime<Utc>>() {
                builder.push(" AND pp.\"expiryDate\" <= ");
                builder.push_bind(dt);
            }
        }
        if join_plan {
            if let Some(device_type) = &filters.device_type {
                builder.push(" AND p.\"deviceType\"::text = ");
                builder.push_bind(device_type);
            }
            if let Some(device_sub_type) = &filters.device_sub_type {
                builder.push(" AND p.\"deviceSubType\"::text = ");
                builder.push_bind(device_sub_type);
            }
        }
    }

    fn apply_post_filters_rows(rows: &mut Vec<PlayerPlanRow>, filters: &PlayerPlanFilterDto) {
        if let Some(min) = filters.min_remaining_usage_count {
            rows.retain(|pp| {
                pp.remaining_usage_count
                    .map(|count| count >= min)
                    .unwrap_or(false)
            });
        }
        if let Some(min) = filters.min_remaining_time_credits {
            rows.retain(|pp| {
                pp.remaining_time_credits
                    .map(|credits| credits >= min)
                    .unwrap_or(false)
            });
        }
        if let Some(is_expired) = filters.is_expired {
            let now = Utc::now();
            if is_expired {
                rows.retain(|pp| pp.expiry_date < now);
            } else {
                rows.retain(|pp| pp.expiry_date >= now);
            }
        }
    }
}

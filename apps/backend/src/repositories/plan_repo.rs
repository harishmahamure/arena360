use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{CreatePlanDto, Plan, PlanFilterDto, UpdatePlanDto};

pub struct PlanRepository {
    pool: PgPool,
}

pub struct PlanCreateValues<'a> {
    pub dto: &'a CreatePlanDto,
    pub duration_minutes: i32,
    pub validity_days: i32,
    pub time_credits: i32,
    pub per_minute_rate: f64,
    pub time_window_start: Option<chrono::NaiveTime>,
    pub time_window_end: Option<chrono::NaiveTime>,
}

impl PlanRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id, name, description,
               price::float8 as price, "planType"::text as plan_type,
               "durationMinutes" as duration_minutes, "validityDays" as validity_days,
               "timeWindowStart" as time_window_start, "timeWindowEnd" as time_window_end,
               "timeCredits" as time_credits, "perMinuteRate"::float8 as per_minute_rate,
               "maxSessions" as max_sessions, "isActive" as is_active,
               "deviceType"::text as device_type, "deviceSubType"::text as device_sub_type,
               "createdAt" as created_at, "updatedAt" as updated_at,
               "deletedAt" as deleted_at
        FROM plans
    "#;

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Plan>, AppError> {
        let query = format!("{} WHERE id = $1 AND \"deletedAt\" IS NULL", Self::SELECT);
        let plan = sqlx::query_as::<_, Plan>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(plan)
    }

    pub async fn find_active(&self) -> Result<Vec<Plan>, AppError> {
        let query = format!(
            "{} WHERE \"isActive\" = true AND \"deletedAt\" IS NULL ORDER BY name ASC",
            Self::SELECT
        );
        let plans = sqlx::query_as::<_, Plan>(&query).fetch_all(&self.pool).await?;
        Ok(plans)
    }

    pub async fn list(&self, filters: &PlanFilterDto) -> Result<PaginationResult<Plan>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT id, name, description,
               price::float8 as price, "planType"::text as plan_type,
               "durationMinutes" as duration_minutes, "validityDays" as validity_days,
               "timeWindowStart" as time_window_start, "timeWindowEnd" as time_window_end,
               "timeCredits" as time_credits, "perMinuteRate"::float8 as per_minute_rate,
               "maxSessions" as max_sessions, "isActive" as is_active,
               "deviceType"::text as device_type, "deviceSubType"::text as device_sub_type,
               "createdAt" as created_at, "updatedAt" as updated_at,
               "deletedAt" as deleted_at
               FROM plans WHERE "deletedAt" IS NULL"#,
        );

        Self::apply_filters(&mut builder, filters);

        let sort_by = filters.sort_by.as_deref().unwrap_or("createdAt");
        let sort_col = match sort_by {
            "name" => "name",
            "price" => "price",
            "planType" => "\"planType\"",
            _ => "\"createdAt\"",
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

        let plans = builder.build_query_as::<Plan>().fetch_all(&self.pool).await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new(r#"SELECT COUNT(*) FROM plans WHERE "deletedAt" IS NULL"#);
        Self::apply_filters(&mut count_builder, filters);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(plans, total.0, page, limit))
    }

    fn apply_filters<'a>(builder: &mut QueryBuilder<'a, Postgres>, filters: &'a PlanFilterDto) {
        if let Some(search) = &filters.search {
            builder.push(" AND (name ILIKE ");
            builder.push_bind(format!("%{search}%"));
            builder.push(" OR description ILIKE ");
            builder.push_bind(format!("%{search}%"));
            builder.push(")");
        }
        if let Some(plan_type) = filters.plan_type.clone() {
            builder.push(" AND \"planType\"::text = ");
            builder.push_bind(plan_type);
        }
        if let Some(is_active) = filters.is_active_bool() {
            builder.push(" AND \"isActive\" = ");
            builder.push_bind(is_active);
        }
        if let Some(min_price) = filters.min_price {
            builder.push(" AND price >= ");
            builder.push_bind(min_price);
        }
        if let Some(max_price) = filters.max_price {
            builder.push(" AND price <= ");
            builder.push_bind(max_price);
        }
        if let Some(device_type) = filters.device_type.clone() {
            builder.push(" AND \"deviceType\"::text = ");
            builder.push_bind(device_type);
        }
        if let Some(device_sub_type) = filters.device_sub_type.clone() {
            builder.push(" AND \"deviceSubType\"::text = ");
            builder.push_bind(device_sub_type);
        }
    }

    pub async fn create(&self, values: PlanCreateValues<'_>) -> Result<Plan, AppError> {
        let dto = values.dto;
        let plan = sqlx::query_as::<_, Plan>(
            r#"
            INSERT INTO plans (
                id, name, description, price, "planType", "durationMinutes", "validityDays",
                "timeWindowStart", "timeWindowEnd", "timeCredits", "perMinuteRate",
                "maxSessions", "isActive", "deviceType", "deviceSubType",
                "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2, $3, $4::plans_plantype_enum, $5, $6, $7, $8, $9, $10,
                $11, COALESCE($12, true), $13::plans_devicetype_enum, $14::plans_devicesubtype_enum, NOW(), NOW()
            )
            RETURNING id, name, description,
                      price::float8 as price, "planType"::text as plan_type,
                      "durationMinutes" as duration_minutes, "validityDays" as validity_days,
                      "timeWindowStart" as time_window_start, "timeWindowEnd" as time_window_end,
                      "timeCredits" as time_credits, "perMinuteRate"::float8 as per_minute_rate,
                      "maxSessions" as max_sessions, "isActive" as is_active,
                      "deviceType"::text as device_type, "deviceSubType"::text as device_sub_type,
                      "createdAt" as created_at, "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(&dto.name)
        .bind(&dto.description)
        .bind(dto.price)
        .bind(&dto.plan_type)
        .bind(values.duration_minutes)
        .bind(values.validity_days)
        .bind(values.time_window_start)
        .bind(values.time_window_end)
        .bind(values.time_credits)
        .bind(values.per_minute_rate)
        .bind(dto.max_sessions)
        .bind(dto.is_active)
        .bind(&dto.device_type)
        .bind(&dto.device_sub_type)
        .fetch_one(&self.pool)
        .await?;

        Ok(plan)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdatePlanDto,
        time_window_start: Option<chrono::NaiveTime>,
        time_window_end: Option<chrono::NaiveTime>,
    ) -> Result<Plan, AppError> {
        let plan = sqlx::query_as::<_, Plan>(
            r#"
            UPDATE plans SET
                name = COALESCE($2, name),
                description = COALESCE($3, description),
                price = COALESCE($4, price),
                "planType" = COALESCE($5::plans_plantype_enum, "planType"),
                "durationMinutes" = COALESCE($6, "durationMinutes"),
                "validityDays" = COALESCE($7, "validityDays"),
                "timeWindowStart" = COALESCE($8, "timeWindowStart"),
                "timeWindowEnd" = COALESCE($9, "timeWindowEnd"),
                "timeCredits" = COALESCE($10, "timeCredits"),
                "perMinuteRate" = COALESCE($11, "perMinuteRate"),
                "maxSessions" = COALESCE($12, "maxSessions"),
                "isActive" = COALESCE($13, "isActive"),
                "deviceType" = COALESCE($14::plans_devicetype_enum, "deviceType"),
                "deviceSubType" = COALESCE($15::plans_devicesubtype_enum, "deviceSubType"),
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id, name, description,
                      price::float8 as price, "planType"::text as plan_type,
                      "durationMinutes" as duration_minutes, "validityDays" as validity_days,
                      "timeWindowStart" as time_window_start, "timeWindowEnd" as time_window_end,
                      "timeCredits" as time_credits, "perMinuteRate"::float8 as per_minute_rate,
                      "maxSessions" as max_sessions, "isActive" as is_active,
                      "deviceType"::text as device_type, "deviceSubType"::text as device_sub_type,
                      "createdAt" as created_at, "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .bind(&dto.name)
        .bind(&dto.description)
        .bind(dto.price)
        .bind(&dto.plan_type)
        .bind(dto.duration_minutes)
        .bind(dto.validity_days)
        .bind(time_window_start)
        .bind(time_window_end)
        .bind(dto.time_credits)
        .bind(dto.per_minute_rate)
        .bind(dto.max_sessions)
        .bind(dto.is_active)
        .bind(&dto.device_type)
        .bind(&dto.device_sub_type)
        .fetch_optional(&self.pool)
        .await?;

        plan.ok_or_else(|| AppError::NotFound(format!("Plan with ID {id} not found")))
    }

    pub async fn deactivate(&self, id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query(
            r#"UPDATE plans SET "isActive" = false, "updatedAt" = NOW()
               WHERE id = $1 AND "deletedAt" IS NULL"#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Plan with ID {id} not found")));
        }
        Ok(())
    }
}

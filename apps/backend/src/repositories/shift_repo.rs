use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{Shift, ShiftFilterDto};

#[derive(Clone)]
pub struct ShiftRepository {
    pool: PgPool,
}

impl ShiftRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id,
               "userId" as user_id,
               "clockIn" as clock_in,
               "clockOut" as clock_out,
               notes,
               status,
               "createdBy" as created_by,
               "updatedBy" as updated_by,
               "createdAt" as created_at,
               "updatedAt" as updated_at
        FROM shifts
    "#;

    pub async fn create(
        &self,
        user_id: Uuid,
        notes: Option<String>,
        actor_id: Uuid,
    ) -> Result<Shift, AppError> {
        let shift = sqlx::query_as::<_, Shift>(
            r#"
            INSERT INTO shifts (id, "userId", "clockIn", notes, status, "createdBy", "updatedBy", "createdAt", "updatedAt")
            VALUES (gen_random_uuid(), $1, NOW(), $2, 'active', $3, $3, NOW(), NOW())
            RETURNING id,
                      "userId" as user_id,
                      "clockIn" as clock_in,
                      "clockOut" as clock_out,
                      notes,
                      status,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(user_id)
        .bind(notes)
        .bind(actor_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(shift)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Shift>, AppError> {
        let query = format!("{} WHERE id = $1", Self::SELECT);
        let shift = sqlx::query_as::<_, Shift>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(shift)
    }

    pub async fn find_active_by_user(&self, user_id: Uuid) -> Result<Option<Shift>, AppError> {
        let query = format!(
            "{} WHERE \"userId\" = $1 AND status = 'active'",
            Self::SELECT
        );
        let shift = sqlx::query_as::<_, Shift>(&query)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(shift)
    }

    pub async fn close(
        &self,
        id: Uuid,
        notes: Option<String>,
        actor_id: Uuid,
    ) -> Result<Shift, AppError> {
        let shift = sqlx::query_as::<_, Shift>(
            r#"
            UPDATE shifts SET
                "clockOut" = NOW(),
                status = 'completed',
                notes = COALESCE($2, notes),
                "updatedBy" = $3,
                "updatedAt" = NOW()
            WHERE id = $1 AND status = 'active'
            RETURNING id,
                      "userId" as user_id,
                      "clockIn" as clock_in,
                      "clockOut" as clock_out,
                      notes,
                      status,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(id)
        .bind(notes)
        .bind(actor_id)
        .fetch_optional(&self.pool)
        .await?;

        shift.ok_or_else(|| AppError::NotFound(format!("Active shift with ID {id} not found")))
    }

    pub async fn force_close(&self, id: Uuid, actor_id: Uuid) -> Result<Shift, AppError> {
        let shift = sqlx::query_as::<_, Shift>(
            r#"
            UPDATE shifts SET
                "clockOut" = NOW(),
                status = 'force_closed',
                "updatedBy" = $2,
                "updatedAt" = NOW()
            WHERE id = $1 AND status = 'active'
            RETURNING id,
                      "userId" as user_id,
                      "clockIn" as clock_in,
                      "clockOut" as clock_out,
                      notes,
                      status,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(id)
        .bind(actor_id)
        .fetch_optional(&self.pool)
        .await?;

        shift.ok_or_else(|| AppError::NotFound(format!("Active shift with ID {id} not found")))
    }

    pub async fn list(
        &self,
        filters: &ShiftFilterDto,
    ) -> Result<PaginationResult<Shift>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT id,
                      "userId" as user_id,
                      "clockIn" as clock_in,
                      "clockOut" as clock_out,
                      notes,
                      status,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
               FROM shifts WHERE 1=1"#,
        );

        Self::apply_filters(&mut builder, filters);

        let sort_by = filters.sort_by.as_deref().unwrap_or("clockIn");
        let sort_col = match sort_by {
            "clockOut" => "\"clockOut\"",
            "status" => "status",
            "createdAt" => "\"createdAt\"",
            _ => "\"clockIn\"",
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
            .build_query_as::<Shift>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM shifts WHERE 1=1");
        Self::apply_filters(&mut count_builder, filters);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(rows, total.0, page, limit))
    }

    fn apply_filters(builder: &mut QueryBuilder<Postgres>, filters: &ShiftFilterDto) {
        if let Some(user_id) = filters.user_id {
            builder.push(" AND \"userId\" = ");
            builder.push_bind(user_id);
        }
        if let Some(status) = &filters.status {
            builder.push(" AND status = ");
            builder.push_bind(status.clone());
        }
        if let Some(from) = filters.clock_in_from {
            builder.push(" AND \"clockIn\" >= ");
            builder.push_bind(from);
        }
        if let Some(to) = filters.clock_in_to {
            builder.push(" AND \"clockIn\" <= ");
            builder.push_bind(to);
        }
    }
}

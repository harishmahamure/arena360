use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CreateDeviceGameDto, DeviceGame, DeviceGameFilterDto, DeviceGameResponse, DeviceGameRow,
};

pub struct DeviceGameRepository {
    pool: PgPool,
}

impl DeviceGameRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT_WITH_RELATIONS: &'static str = r#"
        SELECT dg.id, dg."deviceId" as device_id, dg."gameId" as game_id,
               dg."installationDate" as installation_date, dg."isActive" as is_active,
               dg."createdAt" as created_at, dg."updatedAt" as updated_at,
               d.name as device_name, d."deviceType" as device_type, d.location as device_location,
               g.title as game_title, g.genre as game_genre
        FROM device_games dg
        LEFT JOIN devices d ON d.id = dg."deviceId" AND d."deletedAt" IS NULL
        LEFT JOIN games g ON g.id = dg."gameId" AND g."deletedAt" IS NULL
    "#;

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<DeviceGameResponse>, AppError> {
        let query = format!(
            "{} WHERE dg.id = $1 AND dg.\"deletedAt\" IS NULL",
            Self::SELECT_WITH_RELATIONS
        );
        let row = sqlx::query_as::<_, DeviceGameRow>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| r.into_response()))
    }

    pub async fn list(
        &self,
        filters: &DeviceGameFilterDto,
    ) -> Result<PaginationResult<DeviceGameResponse>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let (rows, total) = self.fetch_filtered(filters, Some(limit), Some(offset)).await?;
        let data = rows.into_iter().map(|r| r.into_response()).collect();
        Ok(PaginationResult::new(data, total, page, limit))
    }

    pub async fn list_by_device(
        &self,
        device_id: Uuid,
        filters: &DeviceGameFilterDto,
    ) -> Result<PaginationResult<DeviceGameResponse>, AppError> {
        let mut filters = filters.clone();
        filters.device_id = Some(device_id);
        self.list(&filters).await
    }

    pub async fn list_by_game(
        &self,
        game_id: Uuid,
        filters: &DeviceGameFilterDto,
    ) -> Result<PaginationResult<DeviceGameResponse>, AppError> {
        let mut filters = filters.clone();
        filters.game_id = Some(game_id);
        self.list(&filters).await
    }

    pub async fn create(&self, dto: &CreateDeviceGameDto) -> Result<DeviceGame, AppError> {
        let device_game = sqlx::query_as::<_, DeviceGame>(
            r#"
            INSERT INTO device_games (
                id, "deviceId", "gameId", "installationDate", "isActive", "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2, COALESCE($3, NOW()), COALESCE($4, true), NOW(), NOW()
            )
            RETURNING id, "deviceId" as device_id, "gameId" as game_id,
                      "installationDate" as installation_date, "isActive" as is_active,
                      "createdAt" as created_at, "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(dto.device_id)
        .bind(dto.game_id)
        .bind(dto.installation_date)
        .bind(dto.is_active)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                AppError::Conflict(
                    "This game is already assigned to the selected device".to_string(),
                )
            } else {
                AppError::Database(e)
            }
        })?;

        Ok(device_game)
    }

    pub async fn soft_delete(&self, id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query(
            r#"UPDATE device_games SET "deletedAt" = NOW() WHERE id = $1 AND "deletedAt" IS NULL"#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "Device-game assignment with ID {id} not found"
            )));
        }
        Ok(())
    }

    pub async fn device_exists(&self, device_id: Uuid) -> Result<bool, AppError> {
        let exists: (bool,) = sqlx::query_as(
            r#"SELECT EXISTS(SELECT 1 FROM devices WHERE id = $1 AND "deletedAt" IS NULL)"#,
        )
        .bind(device_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists.0)
    }

    pub async fn game_exists(&self, game_id: Uuid) -> Result<bool, AppError> {
        let exists: (bool,) = sqlx::query_as(
            r#"SELECT EXISTS(SELECT 1 FROM games WHERE id = $1 AND "deletedAt" IS NULL)"#,
        )
        .bind(game_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists.0)
    }

    async fn fetch_filtered(
        &self,
        filters: &DeviceGameFilterDto,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<(Vec<DeviceGameRow>, i64), AppError> {
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT dg.id, dg.\"deviceId\" as device_id, dg.\"gameId\" as game_id, \
             dg.\"installationDate\" as installation_date, dg.\"isActive\" as is_active, \
             dg.\"createdAt\" as created_at, dg.\"updatedAt\" as updated_at, \
             d.name as device_name, d.\"deviceType\" as device_type, d.location as device_location, \
             g.title as game_title, g.genre as game_genre \
             FROM device_games dg \
             LEFT JOIN devices d ON d.id = dg.\"deviceId\" AND d.\"deletedAt\" IS NULL \
             LEFT JOIN games g ON g.id = dg.\"gameId\" AND g.\"deletedAt\" IS NULL \
             WHERE dg.\"deletedAt\" IS NULL",
        );

        Self::apply_filters(&mut builder, filters);

        let sort_by = filters.sort_by.as_deref().unwrap_or("createdAt");
        let sort_col = match sort_by {
            "installationDate" => "dg.\"installationDate\"",
            "isActive" => "dg.\"isActive\"",
            _ => "dg.\"createdAt\"",
        };
        let sort_order = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        builder.push(format!(" ORDER BY {sort_col} {sort_order}"));

        if let Some(limit) = limit {
            builder.push(" LIMIT ");
            builder.push_bind(limit);
        }
        if let Some(offset) = offset {
            builder.push(" OFFSET ");
            builder.push_bind(offset);
        }

        let rows = builder.build_query_as::<DeviceGameRow>().fetch_all(&self.pool).await?;

        let mut count_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT COUNT(*) FROM device_games dg WHERE dg.\"deletedAt\" IS NULL",
        );
        Self::apply_filters(&mut count_builder, filters);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok((rows, total.0))
    }

    fn apply_filters(builder: &mut QueryBuilder<Postgres>, filters: &DeviceGameFilterDto) {
        if let Some(device_id) = filters.device_id {
            builder.push(" AND dg.\"deviceId\" = ");
            builder.push_bind(device_id);
        }
        if let Some(game_id) = filters.game_id {
            builder.push(" AND dg.\"gameId\" = ");
            builder.push_bind(game_id);
        }
        if let Some(is_active) = filters.is_active {
            builder.push(" AND dg.\"isActive\" = ");
            builder.push_bind(is_active == 1);
        }
    }
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    if let sqlx::Error::Database(db_err) = err {
        db_err.code().as_deref() == Some("23505")
    } else {
        false
    }
}

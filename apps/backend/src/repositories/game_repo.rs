use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{CreateGameDto, Game, GameFilterDto, UpdateGameDto};

#[derive(Clone)]
pub struct GameRepository {
    pool: PgPool,
}

impl GameRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id, name,
               "thumbnailUrl" as thumbnail_url,
               "logoUrl" as logo_url,
               "videoUrl" as video_url,
               "launchRef" as launch_ref,
               "isActive" as is_active,
               "sortOrder" as sort_order,
               "createdBy" as created_by, "updatedBy" as updated_by,
               "createdAt" as created_at, "updatedAt" as updated_at,
               "deletedAt" as deleted_at
        FROM games
    "#;

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Game>, AppError> {
        let query = format!("{} WHERE id = $1 AND \"deletedAt\" IS NULL", Self::SELECT);
        let game = sqlx::query_as::<_, Game>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(game)
    }

    pub async fn list(&self, filters: &GameFilterDto) -> Result<PaginationResult<Game>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(50).clamp(1, 200);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> =
            QueryBuilder::new(format!("{} WHERE \"deletedAt\" IS NULL", Self::SELECT));
        if let Some(active) = filters.is_active {
            builder.push(" AND \"isActive\" = ");
            builder.push_bind(active);
        }
        if let Some(name) = &filters.name {
            builder.push(" AND name ILIKE ");
            builder.push_bind(format!("%{name}%"));
        }

        let sort_col = match filters.sort_by.as_deref() {
            Some("name") => "name",
            Some("createdAt") => "\"createdAt\"",
            _ => "\"sortOrder\"",
        };
        let sort_order = if filters.sort_order.as_deref() == Some("DESC") {
            "DESC"
        } else {
            "ASC"
        };
        builder.push(format!(
            " ORDER BY {sort_col} {sort_order}, name ASC LIMIT "
        ));
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let games = builder
            .build_query_as::<Game>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM games WHERE \"deletedAt\" IS NULL");
        if let Some(active) = filters.is_active {
            count_builder.push(" AND \"isActive\" = ");
            count_builder.push_bind(active);
        }
        if let Some(name) = &filters.name {
            count_builder.push(" AND name ILIKE ");
            count_builder.push_bind(format!("%{name}%"));
        }
        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(games, total.0, page, limit))
    }

    pub async fn create(
        &self,
        dto: &CreateGameDto,
        actor_id: Option<Uuid>,
    ) -> Result<Game, AppError> {
        let game = sqlx::query_as::<_, Game>(
            r#"
            INSERT INTO games (
                id, name, "thumbnailUrl", "logoUrl", "videoUrl", "launchRef",
                "isActive", "sortOrder", "createdBy", "updatedBy", "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2, $3, $4, $5,
                COALESCE($6, TRUE), COALESCE($7, 0), $8, $8, NOW(), NOW()
            )
            RETURNING id, name, "thumbnailUrl" as thumbnail_url, "logoUrl" as logo_url,
                      "videoUrl" as video_url, "launchRef" as launch_ref,
                      "isActive" as is_active, "sortOrder" as sort_order,
                      "createdBy" as created_by, "updatedBy" as updated_by,
                      "createdAt" as created_at, "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(&dto.name)
        .bind(&dto.thumbnail_url)
        .bind(&dto.logo_url)
        .bind(&dto.video_url)
        .bind(&dto.launch_ref)
        .bind(dto.is_active)
        .bind(dto.sort_order)
        .bind(actor_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(game)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdateGameDto,
        actor_id: Option<Uuid>,
    ) -> Result<Game, AppError> {
        let game = sqlx::query_as::<_, Game>(
            r#"
            UPDATE games SET
                name = COALESCE($2, name),
                "thumbnailUrl" = COALESCE($3, "thumbnailUrl"),
                "logoUrl" = COALESCE($4, "logoUrl"),
                "videoUrl" = COALESCE($5, "videoUrl"),
                "launchRef" = COALESCE($6, "launchRef"),
                "isActive" = COALESCE($7, "isActive"),
                "sortOrder" = COALESCE($8, "sortOrder"),
                "updatedBy" = COALESCE($9, "updatedBy"),
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id, name, "thumbnailUrl" as thumbnail_url, "logoUrl" as logo_url,
                      "videoUrl" as video_url, "launchRef" as launch_ref,
                      "isActive" as is_active, "sortOrder" as sort_order,
                      "createdBy" as created_by, "updatedBy" as updated_by,
                      "createdAt" as created_at, "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .bind(&dto.name)
        .bind(&dto.thumbnail_url)
        .bind(&dto.logo_url)
        .bind(&dto.video_url)
        .bind(&dto.launch_ref)
        .bind(dto.is_active)
        .bind(dto.sort_order)
        .bind(actor_id)
        .fetch_optional(&self.pool)
        .await?;

        game.ok_or_else(|| AppError::NotFound(format!("Game with ID {id} not found")))
    }

    pub async fn soft_delete(&self, id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query(
            r#"UPDATE games SET "deletedAt" = NOW() WHERE id = $1 AND "deletedAt" IS NULL"#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Game with ID {id} not found")));
        }
        Ok(())
    }
}

use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{tags_to_db, CreateGameDto, Game, GameFilterDto, UpdateGameDto};

pub struct GameRepository {
    pool: PgPool,
}

impl GameRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id, title, description, genre, "isActive" as is_active,
               "imageUrl" as image_url, "videoUrl" as video_url,
               "trailerUrl" as trailer_url, developer, publisher,
               "releaseDate" as release_date, platform, category,
               "isMultiplayer" as is_multiplayer, "iconUrl" as icon_url,
               "bannerUrl" as banner_url, "thumbnailUrl" as thumbnail_url,
               "backgroundUrl" as background_url, "logoUrl" as logo_url,
               tags, "ageRating" as age_rating, "minPlayers" as min_players,
               "maxPlayers" as max_players,
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
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT id, title, description, genre, "isActive" as is_active,
               "imageUrl" as image_url, "videoUrl" as video_url,
               "trailerUrl" as trailer_url, developer, publisher,
               "releaseDate" as release_date, platform, category,
               "isMultiplayer" as is_multiplayer, "iconUrl" as icon_url,
               "bannerUrl" as banner_url, "thumbnailUrl" as thumbnail_url,
               "backgroundUrl" as background_url, "logoUrl" as logo_url,
               tags, "ageRating" as age_rating, "minPlayers" as min_players,
               "maxPlayers" as max_players,
               "createdAt" as created_at, "updatedAt" as updated_at,
               "deletedAt" as deleted_at
               FROM games WHERE "deletedAt" IS NULL"#,
        );

        Self::apply_filters(&mut builder, filters);

        builder.push(" ORDER BY \"releaseDate\" DESC NULLS LAST LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let games = builder.build_query_as::<Game>().fetch_all(&self.pool).await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new(r#"SELECT COUNT(*) FROM games WHERE "deletedAt" IS NULL"#);
        Self::apply_filters(&mut count_builder, filters);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(games, total.0, page, limit))
    }

    fn apply_filters<'a>(builder: &mut QueryBuilder<'a, Postgres>, filters: &'a GameFilterDto) {
        if let Some(genre) = &filters.genre {
            builder.push(" AND genre = ");
            builder.push_bind(genre);
        }
        if let Some(is_active) = filters.is_active_bool() {
            builder.push(" AND \"isActive\" = ");
            builder.push_bind(is_active);
        }
        if let Some(title) = &filters.title {
            builder.push(" AND title ILIKE ");
            builder.push_bind(format!("%{title}%"));
        }
        if let Some(platform) = &filters.platform {
            builder.push(" AND platform = ");
            builder.push_bind(platform);
        }
        if let Some(category) = &filters.category {
            builder.push(" AND category = ");
            builder.push_bind(category);
        }
        if let Some(developer) = &filters.developer {
            builder.push(" AND developer = ");
            builder.push_bind(developer);
        }
        if let Some(publisher) = &filters.publisher {
            builder.push(" AND publisher = ");
            builder.push_bind(publisher);
        }
        if let Some(is_multiplayer) = filters.is_multiplayer_bool() {
            builder.push(" AND \"isMultiplayer\" = ");
            builder.push_bind(is_multiplayer);
        }
        if let Some(age_rating) = &filters.age_rating {
            builder.push(" AND \"ageRating\" = ");
            builder.push_bind(age_rating);
        }
        if let Some(tag) = &filters.tag {
            builder.push(" AND tags ILIKE ");
            builder.push_bind(format!("%{tag}%"));
        }
    }

    pub async fn create(&self, dto: &CreateGameDto) -> Result<Game, AppError> {
        let tags = tags_to_db(&dto.tags);

        let game = sqlx::query_as::<_, Game>(
            r#"
            INSERT INTO games (
                id, title, description, genre, "isActive",
                "imageUrl", "videoUrl", "trailerUrl", developer, publisher,
                "releaseDate", platform, category, "isMultiplayer",
                "iconUrl", "bannerUrl", "thumbnailUrl", "backgroundUrl", "logoUrl",
                tags, "ageRating", "minPlayers", "maxPlayers",
                "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2, $3, COALESCE($4, true),
                $5, $6, $7, $8, $9, $10, $11, $12, COALESCE($13, false),
                $14, $15, $16, $17, $18, $19, $20, $21, $22, NOW(), NOW()
            )
            RETURNING id, title, description, genre, "isActive" as is_active,
                      "imageUrl" as image_url, "videoUrl" as video_url,
                      "trailerUrl" as trailer_url, developer, publisher,
                      "releaseDate" as release_date, platform, category,
                      "isMultiplayer" as is_multiplayer, "iconUrl" as icon_url,
                      "bannerUrl" as banner_url, "thumbnailUrl" as thumbnail_url,
                      "backgroundUrl" as background_url, "logoUrl" as logo_url,
                      tags, "ageRating" as age_rating, "minPlayers" as min_players,
                      "maxPlayers" as max_players,
                      "createdAt" as created_at, "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(&dto.title)
        .bind(&dto.description)
        .bind(&dto.genre)
        .bind(dto.is_active)
        .bind(&dto.image_url)
        .bind(&dto.video_url)
        .bind(&dto.trailer_url)
        .bind(&dto.developer)
        .bind(&dto.publisher)
        .bind(dto.release_date)
        .bind(&dto.platform)
        .bind(&dto.category)
        .bind(dto.is_multiplayer)
        .bind(&dto.icon_url)
        .bind(&dto.banner_url)
        .bind(&dto.thumbnail_url)
        .bind(&dto.background_url)
        .bind(&dto.logo_url)
        .bind(tags)
        .bind(&dto.age_rating)
        .bind(dto.min_players)
        .bind(dto.max_players)
        .fetch_one(&self.pool)
        .await?;

        Ok(game)
    }

    pub async fn update(&self, id: Uuid, dto: &UpdateGameDto) -> Result<Game, AppError> {
        let tags = dto
            .tags
            .as_ref()
            .and_then(|t| tags_to_db(&Some(t.clone())));

        let game = sqlx::query_as::<_, Game>(
            r#"
            UPDATE games SET
                title = COALESCE($2, title),
                description = COALESCE($3, description),
                genre = COALESCE($4, genre),
                "isActive" = COALESCE($5, "isActive"),
                "imageUrl" = COALESCE($6, "imageUrl"),
                "videoUrl" = COALESCE($7, "videoUrl"),
                "trailerUrl" = COALESCE($8, "trailerUrl"),
                developer = COALESCE($9, developer),
                publisher = COALESCE($10, publisher),
                "releaseDate" = COALESCE($11, "releaseDate"),
                platform = COALESCE($12, platform),
                category = COALESCE($13, category),
                "isMultiplayer" = COALESCE($14, "isMultiplayer"),
                "iconUrl" = COALESCE($15, "iconUrl"),
                "bannerUrl" = COALESCE($16, "bannerUrl"),
                "thumbnailUrl" = COALESCE($17, "thumbnailUrl"),
                "backgroundUrl" = COALESCE($18, "backgroundUrl"),
                "logoUrl" = COALESCE($19, "logoUrl"),
                tags = COALESCE($20, tags),
                "ageRating" = COALESCE($21, "ageRating"),
                "minPlayers" = COALESCE($22, "minPlayers"),
                "maxPlayers" = COALESCE($23, "maxPlayers"),
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id, title, description, genre, "isActive" as is_active,
                      "imageUrl" as image_url, "videoUrl" as video_url,
                      "trailerUrl" as trailer_url, developer, publisher,
                      "releaseDate" as release_date, platform, category,
                      "isMultiplayer" as is_multiplayer, "iconUrl" as icon_url,
                      "bannerUrl" as banner_url, "thumbnailUrl" as thumbnail_url,
                      "backgroundUrl" as background_url, "logoUrl" as logo_url,
                      tags, "ageRating" as age_rating, "minPlayers" as min_players,
                      "maxPlayers" as max_players,
                      "createdAt" as created_at, "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .bind(&dto.title)
        .bind(&dto.description)
        .bind(&dto.genre)
        .bind(dto.is_active)
        .bind(&dto.image_url)
        .bind(&dto.video_url)
        .bind(&dto.trailer_url)
        .bind(&dto.developer)
        .bind(&dto.publisher)
        .bind(dto.release_date)
        .bind(&dto.platform)
        .bind(&dto.category)
        .bind(dto.is_multiplayer)
        .bind(&dto.icon_url)
        .bind(&dto.banner_url)
        .bind(&dto.thumbnail_url)
        .bind(&dto.background_url)
        .bind(&dto.logo_url)
        .bind(tags)
        .bind(&dto.age_rating)
        .bind(dto.min_players)
        .bind(dto.max_players)
        .fetch_optional(&self.pool)
        .await?;

        game.ok_or_else(|| AppError::NotFound(format!("Game with ID {id} not found")))
    }

    pub async fn soft_delete(&self, id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query(
            r#"UPDATE games SET "isActive" = false, "updatedAt" = NOW()
               WHERE id = $1 AND "deletedAt" IS NULL"#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Game with ID {id} not found")));
        }
        Ok(())
    }

    pub async fn title_exists(&self, title: &str, exclude_id: Option<Uuid>) -> Result<bool, AppError> {
        let exists: (bool,) = match exclude_id {
            Some(id) => {
                sqlx::query_as(
                    r#"SELECT EXISTS(
                        SELECT 1 FROM games
                        WHERE LOWER(title) = LOWER($1) AND id != $2 AND "deletedAt" IS NULL
                    )"#,
                )
                .bind(title)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as(
                    r#"SELECT EXISTS(
                        SELECT 1 FROM games
                        WHERE LOWER(title) = LOWER($1) AND "deletedAt" IS NULL
                    )"#,
                )
                .bind(title)
                .fetch_one(&self.pool)
                .await?
            }
        };
        Ok(exists.0)
    }
}

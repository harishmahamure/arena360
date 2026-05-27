use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{CreateUnitDto, Unit, UnitFilterDto, UpdateUnitDto};

pub struct UnitRepository {
    pool: PgPool,
}

impl UnitRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id, name, abbreviation, type::text as type,
               description, "isActive" as is_active,
               "createdBy" as created_by, "updatedBy" as updated_by,
               "createdAt" as created_at, "updatedAt" as updated_at,
               "deletedAt" as deleted_at
        FROM units
    "#;

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Unit>, AppError> {
        let query = format!("{} WHERE id = $1 AND \"deletedAt\" IS NULL", Self::SELECT);
        let unit = sqlx::query_as::<_, Unit>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(unit)
    }

    pub async fn list(&self, filters: &UnitFilterDto) -> Result<PaginationResult<Unit>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, name, abbreviation, type::text as type, description, \
             \"isActive\" as is_active, \
             \"createdBy\" as created_by, \"updatedBy\" as updated_by, \
             \"createdAt\" as created_at, \
             \"updatedAt\" as updated_at, \"deletedAt\" as deleted_at \
             FROM units WHERE \"deletedAt\" IS NULL",
        );

        if let Some(name) = &filters.name {
            builder.push(" AND name ILIKE ");
            builder.push_bind(format!("%{name}%"));
        }
        if let Some(unit_type) = &filters.r#type {
            builder.push(" AND type::text = ");
            builder.push_bind(unit_type);
        }
        if let Some(is_active) = filters.is_active {
            builder.push(" AND \"isActive\" = ");
            builder.push_bind(is_active);
        }

        let sort_order = if filters.sort.as_deref() == Some("desc") {
            "DESC"
        } else {
            "ASC"
        };
        builder.push(format!(" ORDER BY name {sort_order} LIMIT "));
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let units = builder
            .build_query_as::<Unit>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM units WHERE \"deletedAt\" IS NULL");
        if let Some(name) = &filters.name {
            count_builder.push(" AND name ILIKE ");
            count_builder.push_bind(format!("%{name}%"));
        }
        if let Some(unit_type) = &filters.r#type {
            count_builder.push(" AND type::text = ");
            count_builder.push_bind(unit_type);
        }
        if let Some(is_active) = filters.is_active {
            count_builder.push(" AND \"isActive\" = ");
            count_builder.push_bind(is_active);
        }

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(units, total.0, page, limit))
    }

    pub async fn create(
        &self,
        dto: &CreateUnitDto,
        actor_id: Option<Uuid>,
    ) -> Result<Unit, AppError> {
        let unit = sqlx::query_as::<_, Unit>(
            r#"
            INSERT INTO units (
                id, name, abbreviation, type, description, "isActive",
                "createdBy", "updatedBy", "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2, COALESCE($3::units_type_enum, 'other'::units_type_enum), $4,
                COALESCE($5, true), $6, $6, NOW(), NOW()
            )
            RETURNING id, name, abbreviation, type::text as type, description,
                      "isActive" as is_active,
                      "createdBy" as created_by, "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at, "deletedAt" as deleted_at
            "#,
        )
        .bind(&dto.name)
        .bind(&dto.abbreviation)
        .bind(&dto.r#type)
        .bind(&dto.description)
        .bind(dto.is_active)
        .bind(actor_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(unit)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdateUnitDto,
        actor_id: Option<Uuid>,
    ) -> Result<Unit, AppError> {
        let unit = sqlx::query_as::<_, Unit>(
            r#"
            UPDATE units SET
                name = COALESCE($2, name),
                abbreviation = COALESCE($3, abbreviation),
                type = COALESCE($4::units_type_enum, type),
                description = COALESCE($5, description),
                "isActive" = COALESCE($6, "isActive"),
                "updatedBy" = COALESCE($7, "updatedBy"),
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id, name, abbreviation, type::text as type, description,
                      "isActive" as is_active,
                      "createdBy" as created_by, "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at, "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .bind(&dto.name)
        .bind(&dto.abbreviation)
        .bind(&dto.r#type)
        .bind(&dto.description)
        .bind(dto.is_active)
        .bind(actor_id)
        .fetch_optional(&self.pool)
        .await?;

        unit.ok_or_else(|| AppError::NotFound(format!("Unit with ID {id} not found")))
    }

    pub async fn soft_delete(&self, id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query(
            r#"UPDATE units SET "isActive" = false, "updatedAt" = NOW() WHERE id = $1 AND "deletedAt" IS NULL"#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Unit with ID {id} not found")));
        }
        Ok(())
    }

    pub async fn name_exists(
        &self,
        name: &str,
        exclude_id: Option<Uuid>,
    ) -> Result<bool, AppError> {
        let exists: (bool,) = match exclude_id {
            Some(id) => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM units WHERE LOWER(name) = LOWER($1) AND id != $2 AND "deletedAt" IS NULL)"#,
                )
                .bind(name)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM units WHERE LOWER(name) = LOWER($1) AND "deletedAt" IS NULL)"#,
                )
                .bind(name)
                .fetch_one(&self.pool)
                .await?
            }
        };
        Ok(exists.0)
    }

    pub async fn abbreviation_exists(
        &self,
        abbreviation: &str,
        exclude_id: Option<Uuid>,
    ) -> Result<bool, AppError> {
        let exists: (bool,) = match exclude_id {
            Some(id) => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM units WHERE LOWER(abbreviation) = LOWER($1) AND id != $2 AND "deletedAt" IS NULL)"#,
                )
                .bind(abbreviation)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM units WHERE LOWER(abbreviation) = LOWER($1) AND "deletedAt" IS NULL)"#,
                )
                .bind(abbreviation)
                .fetch_one(&self.pool)
                .await?
            }
        };
        Ok(exists.0)
    }
}

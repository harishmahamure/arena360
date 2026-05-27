use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{User, UserFilterDto, UpdateUserDto};

pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT_PUBLIC: &'static str = r#"
        SELECT id, email, username,
               NULL::varchar as password_hash,
               "isActive" as is_active,
               "firstName" as first_name, "lastName" as last_name,
               "phoneNumber" as phone_number, role,
               NULL::varchar as session_otp_id,
               NULL::varchar as session_otp,
               "createdAt" as created_at, "updatedAt" as updated_at,
               "deletedAt" as deleted_at
        FROM users
    "#;

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, AppError> {
        let query = format!(
            "{} WHERE id = $1 AND \"deletedAt\" IS NULL",
            Self::SELECT_PUBLIC
        );
        let user = sqlx::query_as::<_, User>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }

    pub async fn list(&self, filters: &UserFilterDto) -> Result<PaginationResult<User>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, email, username, \
             NULL::varchar as password_hash, \
             \"isActive\" as is_active, \
             \"firstName\" as first_name, \"lastName\" as last_name, \
             \"phoneNumber\" as phone_number, role, \
             NULL::varchar as session_otp_id, \
             NULL::varchar as session_otp, \
             \"createdAt\" as created_at, \"updatedAt\" as updated_at, \
             \"deletedAt\" as deleted_at \
             FROM users WHERE \"deletedAt\" IS NULL",
        );

        Self::apply_filters(&mut builder, filters);

        let sort_by = filters.sort_by.as_deref().unwrap_or("createdAt");
        let sort_col = match sort_by {
            "username" => "username",
            "email" => "email",
            "role" => "role",
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

        let users = builder.build_query_as::<User>().fetch_all(&self.pool).await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM users WHERE \"deletedAt\" IS NULL");
        Self::apply_filters(&mut count_builder, filters);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(users, total.0, page, limit))
    }

    pub async fn create_player(
        &self,
        email: Option<&str>,
        username: &str,
        password_hash: &str,
        first_name: Option<&str>,
        last_name: Option<&str>,
    ) -> Result<User, AppError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (
                id, email, username, password_hash, "firstName", "lastName",
                role, "isActive", "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2, $3, $4, $5,
                'player', true, NOW(), NOW()
            )
            RETURNING id, email, username,
                      NULL::varchar as password_hash,
                      "isActive" as is_active,
                      "firstName" as first_name, "lastName" as last_name,
                      "phoneNumber" as phone_number, role,
                      NULL::varchar as session_otp_id,
                      NULL::varchar as session_otp,
                      "createdAt" as created_at, "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(email)
        .bind(username)
        .bind(password_hash)
        .bind(first_name)
        .bind(last_name)
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn update(&self, id: Uuid, dto: &UpdateUserDto) -> Result<User, AppError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users SET
                email = COALESCE($2, email),
                username = COALESCE($3, username),
                "firstName" = COALESCE($4, "firstName"),
                "lastName" = COALESCE($5, "lastName"),
                role = COALESCE($6, role),
                "isActive" = COALESCE($7, "isActive"),
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id, email, username,
                      NULL::varchar as password_hash,
                      "isActive" as is_active,
                      "firstName" as first_name, "lastName" as last_name,
                      "phoneNumber" as phone_number, role,
                      NULL::varchar as session_otp_id,
                      NULL::varchar as session_otp,
                      "createdAt" as created_at, "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .bind(&dto.email)
        .bind(&dto.username)
        .bind(&dto.first_name)
        .bind(&dto.last_name)
        .bind(&dto.role)
        .bind(dto.is_active)
        .fetch_optional(&self.pool)
        .await?;

        user.ok_or_else(|| AppError::NotFound(format!("User with ID {id} not found")))
    }

    pub async fn username_exists(&self, username: &str, exclude_id: Option<Uuid>) -> Result<bool, AppError> {
        let exists: (bool,) = match exclude_id {
            Some(id) => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM users WHERE LOWER(username) = LOWER($1) AND id != $2 AND "deletedAt" IS NULL)"#,
                )
                .bind(username)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM users WHERE LOWER(username) = LOWER($1) AND "deletedAt" IS NULL)"#,
                )
                .bind(username)
                .fetch_one(&self.pool)
                .await?
            }
        };
        Ok(exists.0)
    }

    pub async fn find_by_username_with_password(
        &self,
        username: &str,
    ) -> Result<Option<User>, AppError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, email, username, password_hash, "isActive" as is_active,
                   "firstName" as first_name, "lastName" as last_name,
                   "phoneNumber" as phone_number, role,
                   "sessionOtpId" as session_otp_id, "sessionOtp" as session_otp,
                   "createdAt" as created_at, "updatedAt" as updated_at,
                   "deletedAt" as deleted_at
            FROM users
            WHERE username = $1 AND "deletedAt" IS NULL
            "#,
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn find_by_session_otp_id(&self, session_otp_id: &str) -> Result<Option<User>, AppError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, email, username, password_hash, "isActive" as is_active,
                   "firstName" as first_name, "lastName" as last_name,
                   "phoneNumber" as phone_number, role,
                   "sessionOtpId" as session_otp_id, "sessionOtp" as session_otp,
                   "createdAt" as created_at, "updatedAt" as updated_at,
                   "deletedAt" as deleted_at
            FROM users
            WHERE "sessionOtpId" = $1 AND "deletedAt" IS NULL
            "#,
        )
        .bind(session_otp_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn update_session_otp(
        &self,
        user_id: Uuid,
        session_otp_id: &str,
        otp: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE users
            SET "sessionOtpId" = $1, "sessionOtp" = $2, "updatedAt" = NOW()
            WHERE id = $3
            "#,
        )
        .bind(session_otp_id)
        .bind(otp)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    fn apply_filters(builder: &mut QueryBuilder<Postgres>, filters: &UserFilterDto) {
        if let Some(email) = &filters.email {
            builder.push(" AND email ILIKE ");
            builder.push_bind(format!("%{email}%"));
        }
        if let Some(username) = &filters.username {
            builder.push(" AND username ILIKE ");
            builder.push_bind(format!("%{username}%"));
        }
        if let Some(role) = filters.role.clone() {
            builder.push(" AND role = ");
            builder.push_bind(role);
        }
        if let Some(is_active) = filters.is_active {
            builder.push(" AND \"isActive\" = ");
            builder.push_bind(is_active == 1);
        }
    }
}

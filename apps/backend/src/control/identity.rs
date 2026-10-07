//! Global staff credentials and profile changes stay in the control plane.
use crate::{
    error::AppError,
    models::{TotpSetupResponseDto, UpdateUserDto},
    services::totp_util::{generate_totp_setup, verify_totp_code},
    validation::{normalize_phone_digits, trim_optional_string, validate_username},
};
use sqlx::PgPool;
use uuid::Uuid;

pub struct IdentityRepository {
    pool: PgPool,
}
impl IdentityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub async fn kiosk_staff_identity(
        &self,
        tenant: Uuid,
        username: &str,
    ) -> Result<Option<crate::models::User>, AppError> {
        Ok(sqlx::query_as("SELECT u.id,u.email,u.username,u.password_hash,u.is_active,u.first_name,u.last_name,u.phone_number,m.role,0::float8 AS credit_limit,NULL::text AS session_otp_id,NULL::text AS session_otp,u.totp_secret,u.totp_enabled,NULL::uuid AS created_by,NULL::uuid AS updated_by,u.created_at,u.updated_at,u.deleted_at,u.avatar_url FROM users u JOIN organization_memberships m ON m.user_id=u.id JOIN tenants t ON t.id=m.tenant_id WHERE m.tenant_id=$1 AND m.role='staff' AND m.is_active AND u.is_active AND u.deleted_at IS NULL AND t.state='ACTIVE' AND lower(u.username)=lower($2)")
            .bind(tenant).bind(username).fetch_optional(&self.pool).await?)
    }
    pub async fn create_disabled_staff(
        &self,
        tenant: Uuid,
        actor: Uuid,
        username: &str,
        password: &str,
    ) -> Result<Uuid, AppError> {
        let mut tx = self.pool.begin().await?;
        // Serialize same-name attempts, including retry after an unknown outcome.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,43))")
            .bind(username)
            .execute(&mut *tx)
            .await?;
        let existing: Option<(Uuid, String)> =
            sqlx::query_as("SELECT id,password_hash FROM users WHERE lower(username)=lower($1)")
                .bind(username)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some((id, hash)) = existing {
            let pending: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pending_staff_creations WHERE user_id=$1 AND tenant_id=$2 AND actor_id=$3)").bind(id).bind(tenant).bind(actor).fetch_one(&mut *tx).await?;
            if !pending
                || !bcrypt::verify(password, &hash)
                    .map_err(|e| AppError::Internal(e.to_string()))?
            {
                return Err(AppError::Conflict("Username is already in use".into()));
            }
            tx.commit().await?;
            return Ok(id);
        }
        let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let id = Uuid::now_v7();
        sqlx::query("INSERT INTO users(id,username,password_hash) VALUES($1,$2,$3)")
            .bind(id)
            .bind(username)
            .bind(hash)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO organization_memberships(tenant_id,user_id,role,is_active) VALUES($1,$2,'staff',false)").bind(tenant).bind(id).execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO pending_staff_creations(user_id,tenant_id,actor_id) VALUES($1,$2,$3)",
        )
        .bind(id)
        .bind(tenant)
        .bind(actor)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(id)
    }
    pub async fn fill_mfa_status(
        &self,
        tenant: Uuid,
        users: &mut [crate::models::User],
    ) -> Result<(), AppError> {
        let ids: Vec<Uuid> = users
            .iter()
            .filter(|u| matches!(u.role.as_deref(), Some("admin" | "staff")))
            .map(|u| u.id)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let rows: Vec<(Uuid,bool)> = sqlx::query_as("SELECT u.id,u.totp_enabled FROM users u JOIN organization_memberships m ON m.user_id=u.id WHERE m.tenant_id=$1 AND u.id=ANY($2) AND u.deleted_at IS NULL")
            .bind(tenant).bind(ids).fetch_all(&self.pool).await?;
        for user in users {
            if let Some((_, enabled)) = rows.iter().find(|r| r.0 == user.id) {
                user.totp_enabled = *enabled;
            }
        }
        Ok(())
    }
    async fn require_member(&self, tenant: Uuid, user: Uuid) -> Result<(), AppError> {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users u JOIN organization_memberships m ON m.user_id=u.id WHERE u.id=$1 AND m.tenant_id=$2 AND u.deleted_at IS NULL)")
            .bind(user).bind(tenant).fetch_one(&self.pool).await?;
        if exists {
            Ok(())
        } else {
            Err(AppError::NotFound(
                "Global staff member not found in this tenant".into(),
            ))
        }
    }
    pub async fn update_profile(
        &self,
        tenant: Uuid,
        user: Uuid,
        dto: UpdateUserDto,
    ) -> Result<(), AppError> {
        self.require_member(tenant, user).await?;
        if dto.is_active.is_some() {
            return Err(AppError::Forbidden(
                "Manage activation through Access management".into(),
            ));
        }
        let username = dto
            .username
            .as_deref()
            .map(validate_username)
            .transpose()?
            .map(|v| v.to_lowercase());
        let phone = dto.phone_number.as_deref().map(normalize_phone_digits);
        let first = trim_optional_string(dto.first_name);
        let last = trim_optional_string(dto.last_name);
        sqlx::query("UPDATE users SET username=COALESCE($2,username),phone_number=COALESCE($3,phone_number),first_name=COALESCE($4,first_name),last_name=COALESCE($5,last_name),updated_at=now() WHERE id=$1")
            .bind(user).bind(username).bind(phone).bind(first).bind(last).execute(&self.pool).await.map_err(|e| if matches!(&e,sqlx::Error::Database(d) if d.is_unique_violation()) {AppError::Conflict("Username already in use".into())} else {e.into()})?;
        Ok(())
    }
    pub async fn change_password(
        &self,
        tenant: Uuid,
        user: Uuid,
        password: &str,
    ) -> Result<(), AppError> {
        self.require_member(tenant, user).await?;
        if !(8..=72).contains(&password.len()) {
            return Err(AppError::BadRequest("Use an 8–72 byte password".into()));
        }
        let admin: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM organization_memberships WHERE user_id=$1 AND role='admin')").bind(user).fetch_one(&self.pool).await?;
        if admin {
            return Err(AppError::Forbidden(
                "Cannot change admin passwords via this endpoint".into(),
            ));
        }
        let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        sqlx::query("UPDATE users SET password_hash=$2,updated_at=now() WHERE id=$1")
            .bind(user)
            .bind(hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn set_avatar(
        &self,
        tenant: Uuid,
        user: Uuid,
        url: Option<&str>,
    ) -> Result<(), AppError> {
        self.require_member(tenant, user).await?;
        sqlx::query("UPDATE users SET avatar_url=$2,updated_at=now() WHERE id=$1")
            .bind(user)
            .bind(url)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn setup_totp(
        &self,
        tenant: Uuid,
        user: Uuid,
    ) -> Result<TotpSetupResponseDto, AppError> {
        self.require_member(tenant, user).await?;
        let (name, enabled): (String, bool) =
            sqlx::query_as("SELECT username,totp_enabled FROM users WHERE id=$1")
                .bind(user)
                .fetch_one(&self.pool)
                .await?;
        let (secret, uri) = generate_totp_setup(&name)?;
        sqlx::query("UPDATE users SET totp_pending_secret=$2,updated_at=now() WHERE id=$1")
            .bind(user)
            .bind(&secret)
            .execute(&self.pool)
            .await?;
        Ok(TotpSetupResponseDto {
            secret,
            otpauthUri: uri,
            totpEnabled: enabled,
        })
    }
    pub async fn verify_totp_setup(
        &self,
        tenant: Uuid,
        user: Uuid,
        code: &str,
    ) -> Result<TotpSetupResponseDto, AppError> {
        self.require_member(tenant, user).await?;
        let mut tx = self.pool.begin().await?;
        let (name, secret): (String, Option<String>) =
            sqlx::query_as("SELECT username,totp_pending_secret FROM users WHERE id=$1 FOR UPDATE")
                .bind(user)
                .fetch_one(&mut *tx)
                .await?;
        let secret =
            secret.ok_or_else(|| AppError::BadRequest("TOTP has not been set up".into()))?;
        if !verify_totp_code(&secret, code, &name)? {
            return Err(AppError::Unauthorized("Invalid TOTP code".into()));
        }
        sqlx::query("UPDATE users SET totp_secret=totp_pending_secret,totp_pending_secret=NULL,totp_enabled=true,updated_at=now() WHERE id=$1").bind(user).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(TotpSetupResponseDto {
            secret,
            otpauthUri: String::new(),
            totpEnabled: true,
        })
    }
    pub async fn disable_totp(&self, tenant: Uuid, user: Uuid) -> Result<(), AppError> {
        self.require_member(tenant, user).await?;
        sqlx::query("UPDATE users SET totp_enabled=false,totp_secret=NULL,totp_pending_secret=NULL,updated_at=now() WHERE id=$1").bind(user).execute(&self.pool).await?;
        Ok(())
    }
}

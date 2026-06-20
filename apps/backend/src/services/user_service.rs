use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, get_or_set, keys, CacheService};
use crate::dto::{JwtUserClaims, PaginationResult, RegisterDto, RegisterResponseDto};
use crate::error::AppError;
use crate::models::{UpdateUserDto, User, UserFilterDto};
use crate::repositories::{CreatePlayerParams, UserRepository};
use crate::services::totp_util::{generate_totp_setup, verify_totp_code};
use crate::validation::{
    normalize_phone_digits, trim_optional_string, trim_secret, validate_username,
};

pub struct UserService {
    repo: UserRepository,
    cache: Arc<dyn CacheService>,
}

impl UserService {
    pub fn new(pool: PgPool, cache: Arc<dyn CacheService>) -> Self {
        Self {
            repo: UserRepository::new(pool),
            cache,
        }
    }

    async fn invalidate_user(&self, user: &User) -> Result<(), AppError> {
        cache::invalidate(
            &*self.cache,
            &[
                keys::user_id(&user.id),
                keys::user_username(&user.username),
            ],
        )
        .await
    }

    pub async fn list(&self, filters: UserFilterDto) -> Result<PaginationResult<User>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<User, AppError> {
        let cache_key = keys::user_id(&id);
        get_or_set(&*self.cache, &cache_key, keys::ttl::AUTH, || async {
            self.repo
                .find_by_id(id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("User with ID {id} not found")))
        })
        .await
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: UpdateUserDto,
        actor_id: Option<Uuid>,
    ) -> Result<User, AppError> {
        let mut dto = dto;
        if let Some(username) = dto.username.take() {
            let normalized = validate_username(&username)?;
            if self.repo.username_exists(&normalized, Some(id)).await? {
                return Err(AppError::Conflict(format!(
                    "User with username '{normalized}' already exists"
                )));
            }
            dto.username = Some(normalized);
        }
        if let Some(phone) = dto.phone_number.take() {
            dto.phone_number = Some(normalize_phone_digits(&phone));
        }
        dto.first_name = trim_optional_string(dto.first_name);
        dto.last_name = trim_optional_string(dto.last_name);
        let user = self.repo.update(id, &dto, actor_id).await?;
        self.invalidate_user(&user).await?;
        Ok(user)
    }

    pub async fn register(
        &self,
        dto: RegisterDto,
        claims: &JwtUserClaims,
    ) -> Result<RegisterResponseDto, AppError> {
        let username = validate_username(&dto.username)?;
        if self.repo.username_exists(&username, None).await? {
            return Err(AppError::Conflict(format!(
                "User with username '{username}' already exists"
            )));
        }

        let role = dto.role.as_deref().unwrap_or("player");
        if role == "admin" {
            return Err(AppError::BadRequest(
                "Cannot register users with admin role".to_string(),
            ));
        }
        if role == "staff" && !claims.is_admin() {
            return Err(AppError::Forbidden(
                "Only admins can create staff accounts".to_string(),
            ));
        }
        if role != "player" && role != "staff" {
            return Err(AppError::BadRequest(format!(
                "Invalid role '{role}'. Allowed values: player, staff"
            )));
        }

        let password = trim_secret(&dto.password);
        let password_hash = bcrypt::hash(&password, bcrypt::DEFAULT_COST)
            .map_err(|e| AppError::Internal(format!("Failed to hash password: {e}")))?;

        let phone_number = normalize_phone_digits(&dto.phoneNumber);
        let actor_id = claims.user_id_uuid();

        self.repo
            .create_player(CreatePlayerParams {
                username: &username,
                password_hash: &password_hash,
                phone_number: &phone_number,
                first_name: trim_optional_string(dto.firstName).as_deref(),
                last_name: trim_optional_string(dto.lastName).as_deref(),
                role,
                actor_id,
            })
            .await?;

        Ok(RegisterResponseDto {
            message: "Created successfully".to_string(),
        })
    }

    pub async fn change_password(
        &self,
        target_user_id: Uuid,
        new_password: &str,
        caller_claims: &JwtUserClaims,
    ) -> Result<(), AppError> {
        if new_password.len() < 8 {
            return Err(AppError::BadRequest(
                "Password must be at least 8 characters".to_string(),
            ));
        }

        let target = self.get_by_id(target_user_id).await?;
        let target_role = target.role.as_deref().unwrap_or("player");

        if target_role == "admin" {
            return Err(AppError::Forbidden(
                "Cannot change admin passwords via this endpoint".to_string(),
            ));
        }

        if target_role == "staff" && !caller_claims.is_admin() {
            return Err(AppError::Forbidden(
                "Only admins can change staff passwords".to_string(),
            ));
        }

        let password_hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)
            .map_err(|e| AppError::Internal(format!("Failed to hash password: {e}")))?;

        self.repo
            .update_password(target_user_id, &password_hash)
            .await
    }

    pub async fn setup_totp(
        &self,
        user_id: Uuid,
    ) -> Result<crate::models::TotpSetupResponseDto, AppError> {
        let user = self.get_by_id(user_id).await?;
        if user.role.as_deref() != Some("staff") && user.role.as_deref() != Some("admin") {
            return Err(AppError::BadRequest(
                "TOTP can only be configured for admin or staff users".to_string(),
            ));
        }

        let (secret, uri) = generate_totp_setup(&user.username)?;
        self.repo.set_totp_secret(user_id, &secret).await?;

        Ok(crate::models::TotpSetupResponseDto {
            secret,
            otpauthUri: uri,
            totpEnabled: user.totp_enabled,
        })
    }

    pub async fn verify_totp_setup(
        &self,
        user_id: Uuid,
        code: &str,
    ) -> Result<crate::models::TotpSetupResponseDto, AppError> {
        let user = self
            .repo
            .find_by_id_for_auth(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

        let Some(secret) = user.totp_secret.as_deref() else {
            return Err(AppError::BadRequest(
                "TOTP has not been set up for this user".to_string(),
            ));
        };

        if !verify_totp_code(secret, code, &user.username)? {
            return Err(AppError::Unauthorized("Invalid TOTP code".to_string()));
        }

        self.repo.enable_totp(user_id).await?;

        Ok(crate::models::TotpSetupResponseDto {
            secret: secret.to_string(),
            otpauthUri: String::new(),
            totpEnabled: true,
        })
    }

    pub async fn disable_totp(&self, user_id: Uuid) -> Result<(), AppError> {
        self.repo.clear_totp(user_id).await
    }

    pub async fn verify_staff_totp(&self, user_id: Uuid, code: &str) -> Result<(), AppError> {
        let user = self
            .repo
            .find_by_id_for_auth(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

        if !user.totp_enabled {
            return Err(AppError::BadRequest(
                "Staff member does not have TOTP enabled".to_string(),
            ));
        }

        let Some(secret) = user.totp_secret.as_deref() else {
            return Err(AppError::BadRequest(
                "Staff member does not have TOTP configured".to_string(),
            ));
        };

        if verify_totp_code(secret, code, &user.username)? {
            Ok(())
        } else {
            Err(AppError::Unauthorized("Invalid TOTP code".to_string()))
        }
    }
}

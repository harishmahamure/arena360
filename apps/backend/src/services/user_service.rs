use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::cache::{self, get_or_set, keys, set_json, CacheService};
use crate::dto::{
    JwtUserClaims, KioskRegisterDto, KioskRegisterResponseDto, PaginationResult, RegisterDto,
    RegisterResponseDto,
};
use crate::error::AppError;
use crate::models::{UpdateUserDto, User, UserFilterDto};
use crate::repositories::{
    CreatePlayerParams, StaffProjectionResult, TenantCreatePlayer, TenantStaffProjection,
    TenantUserRepository, UserRepository,
};
use crate::services::totp_util::{generate_totp_setup, verify_totp_code};
use crate::tenancy::TenantDb;
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

    async fn invalidate_user_caches(
        &self,
        user: &User,
        previous_username: Option<&str>,
    ) -> Result<(), AppError> {
        let mut cache_keys = vec![keys::user_id(&user.id), keys::user_username(&user.username)];
        if let Some(old) = previous_username {
            if old != user.username {
                cache_keys.push(keys::user_username(old));
            }
        }
        cache::invalidate(&*self.cache, &cache_keys).await?;
        self.cache.invalidate_prefix("users:list:").await
    }

    fn user_public_profile(user: &User) -> User {
        let mut public = user.clone();
        public.password_hash = None;
        public.session_otp_id = None;
        public.session_otp = None;
        public.totp_secret = None;
        public
    }

    async fn warm_public_id_cache(&self, user: &User) -> Result<(), AppError> {
        let public = Self::user_public_profile(user);
        set_json(
            &*self.cache,
            &keys::user_id(&user.id),
            &public,
            keys::ttl::AUTH,
        )
        .await
    }

    /// Auth lookup by username with cache-aside on `user:username:*`.
    ///
    /// Cached values include bcrypt hashes and TOTP secrets (same trust boundary as Postgres;
    /// ADR-0003). Misses are not cached. On hit, also warms `user:id:{uuid}` without secrets.
    pub async fn find_by_username_for_auth(
        &self,
        username: &str,
    ) -> Result<Option<User>, AppError> {
        let cache_key = keys::user_username(username);

        if let Some(cached) =
            cache::get_json::<AuthUserCacheEntry>(&*self.cache, &cache_key).await?
        {
            let user = User::from(cached);
            if user.password_hash.is_some() {
                self.warm_public_id_cache(&user).await?;
                return Ok(Some(user));
            }
            // Self-heal entries written before auth cache used a dedicated shape.
            let _ = self.cache.delete(&[&cache_key]).await;
        }

        let user = self.repo.find_by_username_with_password(username).await?;

        if let Some(ref found) = user {
            set_json(
                &*self.cache,
                &cache_key,
                &AuthUserCacheEntry::from(found),
                keys::ttl::AUTH,
            )
            .await?;
            self.warm_public_id_cache(found).await?;
        }

        Ok(user)
    }

    pub async fn find_by_username_for_auth_tenant(
        &self,
        db: Arc<TenantDb>,
        username: &str,
    ) -> Result<Option<User>, AppError> {
        TenantUserRepository::new(db)
            .find_player_for_auth(username)
            .await
    }

    pub async fn list_tenant(
        &self,
        db: Arc<TenantDb>,
        filters: UserFilterDto,
    ) -> Result<PaginationResult<User>, AppError> {
        TenantUserRepository::new(db).list(&filters).await
    }

    pub async fn get_by_id_tenant(&self, db: Arc<TenantDb>, id: Uuid) -> Result<User, AppError> {
        TenantUserRepository::new(db)
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("User with ID {id} not found")))
    }

    pub async fn register_tenant(
        &self,
        db: Arc<TenantDb>,
        dto: RegisterDto,
        claims: &JwtUserClaims,
    ) -> Result<RegisterResponseDto, AppError> {
        let role = dto.role.as_deref().unwrap_or("player");
        if role != "player" {
            return Err(AppError::BadRequest(
                "Tenant-local registration only creates players".into(),
            ));
        }
        let username = validate_username(&dto.username)?;
        let repo = TenantUserRepository::new(db);
        if repo.username_exists(&username, None).await? {
            return Err(AppError::Conflict(format!(
                "User with username '{username}' already exists"
            )));
        }
        let password = trim_secret(&dto.password);
        let password_hash = bcrypt::hash(&password, bcrypt::DEFAULT_COST)
            .map_err(|error| AppError::Internal(format!("Failed to hash password: {error}")))?;
        repo.create_player(TenantCreatePlayer {
            username,
            password_hash,
            phone_number: normalize_phone_digits(&dto.phoneNumber),
            first_name: trim_optional_string(dto.firstName),
            last_name: trim_optional_string(dto.lastName),
            actor_id: claims.user_id_uuid(),
        })
        .await?;
        Ok(RegisterResponseDto {
            message: "Created successfully".into(),
        })
    }

    pub async fn register_from_kiosk_tenant(
        &self,
        db: Arc<TenantDb>,
        dto: KioskRegisterDto,
    ) -> Result<KioskRegisterResponseDto, AppError> {
        let username = match validate_username(&dto.username) {
            Ok(value) => value,
            Err(AppError::BadRequest(message)) => {
                return Err(AppError::bad_request_code(
                    &message,
                    Some(serde_json::json!({ "field": "username" })),
                ));
            }
            Err(error) => return Err(error),
        };
        let repo = TenantUserRepository::new(db);
        if repo.username_exists(&username, None).await? {
            return Err(AppError::conflict_code(
                "AUTH_USERNAME_ALREADY_EXISTS",
                Some(serde_json::json!({ "field": "username" })),
            ));
        }
        let password = trim_secret(&dto.password);
        if password.len() < 8 {
            return Err(AppError::bad_request_code(
                "AUTH_WEAK_PASSWORD",
                Some(serde_json::json!({ "field": "password" })),
            ));
        }
        let phone_number = normalize_phone_digits(&dto.phoneNumber);
        if phone_number.len() < 10 {
            return Err(AppError::bad_request_code(
                "Phone number must be at least 10 digits",
                Some(serde_json::json!({ "field": "phoneNumber" })),
            ));
        }
        let password_hash = bcrypt::hash(&password, bcrypt::DEFAULT_COST)
            .map_err(|error| AppError::Internal(format!("Failed to hash password: {error}")))?;
        let user = repo
            .create_player(TenantCreatePlayer {
                username,
                password_hash,
                phone_number,
                first_name: trim_optional_string(dto.firstName),
                last_name: trim_optional_string(dto.lastName),
                actor_id: None,
            })
            .await?;
        Ok(KioskRegisterResponseDto {
            message: "Account created successfully".into(),
            username: user.username,
            userId: user.id.to_string(),
        })
    }

    pub async fn update_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
        mut dto: UpdateUserDto,
        actor_id: Option<Uuid>,
    ) -> Result<User, AppError> {
        if dto.role.as_deref().is_some_and(|role| role != "player") {
            return Err(AppError::BadRequest(
                "Tenant-local user roles cannot be changed".into(),
            ));
        }
        dto.role = None;
        let repo = TenantUserRepository::new(db);
        let _current = repo
            .find_by_id(id)
            .await?
            .filter(|user| user.role.as_deref() == Some("player"))
            .ok_or_else(|| AppError::NotFound(format!("User with ID {id} not found")))?;
        if let Some(username) = dto.username.take() {
            let username = validate_username(&username)?;
            if repo.username_exists(&username, Some(id)).await? {
                return Err(AppError::Conflict(format!(
                    "User with username '{username}' already exists"
                )));
            }
            dto.username = Some(username);
        }
        if let Some(phone) = dto.phone_number.take() {
            dto.phone_number = Some(normalize_phone_digits(&phone));
        }
        dto.first_name = trim_optional_string(dto.first_name);
        dto.last_name = trim_optional_string(dto.last_name);
        repo.update(id, &dto, actor_id).await
    }

    pub async fn change_password_tenant(
        &self,
        db: Arc<TenantDb>,
        target_user_id: Uuid,
        new_password: &str,
        _caller_claims: &JwtUserClaims,
    ) -> Result<(), AppError> {
        if new_password.len() < 8 {
            return Err(AppError::BadRequest(
                "Password must be at least 8 characters".into(),
            ));
        }
        let repo = TenantUserRepository::new(db);
        let _target = repo
            .find_by_id(target_user_id)
            .await?
            .filter(|user| user.role.as_deref() == Some("player"))
            .ok_or_else(|| {
                AppError::NotFound(format!("User with ID {target_user_id} not found"))
            })?;
        let password_hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)
            .map_err(|error| AppError::Internal(format!("Failed to hash password: {error}")))?;
        repo.update_password(target_user_id, &password_hash).await
    }

    pub async fn set_avatar_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
        avatar_url: Option<&str>,
    ) -> Result<User, AppError> {
        TenantUserRepository::new(db)
            .set_avatar(id, avatar_url)
            .await
    }

    pub async fn project_staff_tenant(
        &self,
        db: Arc<TenantDb>,
        projection: TenantStaffProjection,
    ) -> Result<StaffProjectionResult, AppError> {
        TenantUserRepository::new(db)
            .project_staff(projection)
            .await
    }

    pub async fn list(&self, filters: UserFilterDto) -> Result<PaginationResult<User>, AppError> {
        let cache_key = keys::users_list(&keys::filter_hash(&filters));
        get_or_set(&*self.cache, &cache_key, keys::ttl::LOOKUP, || async {
            self.repo.list(&filters).await
        })
        .await
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

    /// Set or clear the caller's own profile photo (DRAFT-0042).
    pub async fn set_avatar(&self, id: Uuid, avatar_url: Option<&str>) -> Result<User, AppError> {
        self.repo.set_avatar(id, avatar_url).await?;
        let user = self
            .repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("User with ID {id} not found")))?;
        self.invalidate_user_caches(&user, None).await?;
        Ok(user)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: UpdateUserDto,
        actor_id: Option<Uuid>,
    ) -> Result<User, AppError> {
        let previous = self.get_by_id(id).await?;
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
        self.invalidate_user_caches(&user, Some(&previous.username))
            .await?;
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
        if role == "staff" && !crate::access::has(claims, "access:manage") {
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

        let user = self
            .repo
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

        self.invalidate_user_caches(&user, None).await?;
        let _ = cache::invalidate_stats(&*self.cache).await;

        Ok(RegisterResponseDto {
            message: "Created successfully".to_string(),
        })
    }

    pub async fn register_from_kiosk(
        &self,
        dto: KioskRegisterDto,
    ) -> Result<KioskRegisterResponseDto, AppError> {
        let username = match validate_username(&dto.username) {
            Ok(value) => value,
            Err(AppError::BadRequest(message)) => {
                return Err(AppError::bad_request_code(
                    &message,
                    Some(serde_json::json!({ "field": "username" })),
                ));
            }
            Err(err) => return Err(err),
        };

        if self.repo.username_exists(&username, None).await? {
            return Err(AppError::conflict_code(
                "AUTH_USERNAME_ALREADY_EXISTS",
                Some(serde_json::json!({ "field": "username" })),
            ));
        }

        let password = trim_secret(&dto.password);
        if password.len() < 8 {
            return Err(AppError::bad_request_code(
                "AUTH_WEAK_PASSWORD",
                Some(serde_json::json!({ "field": "password" })),
            ));
        }

        let phone_number = normalize_phone_digits(&dto.phoneNumber);
        if phone_number.len() < 10 {
            return Err(AppError::bad_request_code(
                "Phone number must be at least 10 digits",
                Some(serde_json::json!({ "field": "phoneNumber" })),
            ));
        }

        let password_hash = bcrypt::hash(&password, bcrypt::DEFAULT_COST)
            .map_err(|e| AppError::Internal(format!("Failed to hash password: {e}")))?;

        let user = self
            .repo
            .create_player(CreatePlayerParams {
                username: &username,
                password_hash: &password_hash,
                phone_number: &phone_number,
                first_name: trim_optional_string(dto.firstName).as_deref(),
                last_name: trim_optional_string(dto.lastName).as_deref(),
                role: "player",
                actor_id: None,
            })
            .await?;

        self.invalidate_user_caches(&user, None).await?;
        let _ = cache::invalidate_stats(&*self.cache).await;

        Ok(KioskRegisterResponseDto {
            message: "Account created successfully".to_string(),
            username: user.username,
            userId: user.id.to_string(),
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

        if target_role == "staff" && !crate::access::has(caller_claims, "team:write") {
            return Err(AppError::Forbidden(
                "Only admins can change staff passwords".to_string(),
            ));
        }

        let password_hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)
            .map_err(|e| AppError::Internal(format!("Failed to hash password: {e}")))?;

        self.repo
            .update_password(target_user_id, &password_hash)
            .await?;

        self.invalidate_user_caches(&target, None).await
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

        self.invalidate_user_caches(&user, None).await?;

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

        self.invalidate_user_caches(&user, None).await?;

        Ok(crate::models::TotpSetupResponseDto {
            secret: secret.to_string(),
            otpauthUri: String::new(),
            totpEnabled: true,
        })
    }

    pub async fn disable_totp(&self, user_id: Uuid) -> Result<(), AppError> {
        let user = self.get_by_id(user_id).await?;
        self.repo.clear_totp(user_id).await?;
        self.invalidate_user_caches(&user, None).await
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

/// Redis auth cache row — includes secrets omitted from API-facing `User` JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuthUserCacheEntry {
    id: Uuid,
    email: Option<String>,
    username: String,
    password_hash: Option<String>,
    is_active: bool,
    first_name: Option<String>,
    last_name: Option<String>,
    phone_number: Option<String>,
    role: Option<String>,
    credit_limit: f64,
    session_otp_id: Option<String>,
    session_otp: Option<String>,
    totp_secret: Option<String>,
    totp_enabled: bool,
    created_by: Option<Uuid>,
    updated_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
    #[serde(default)]
    avatar_url: Option<String>,
}

impl From<&User> for AuthUserCacheEntry {
    fn from(user: &User) -> Self {
        Self {
            id: user.id,
            email: user.email.clone(),
            username: user.username.clone(),
            password_hash: user.password_hash.clone(),
            is_active: user.is_active,
            first_name: user.first_name.clone(),
            last_name: user.last_name.clone(),
            phone_number: user.phone_number.clone(),
            role: user.role.clone(),
            credit_limit: user.credit_limit,
            session_otp_id: user.session_otp_id.clone(),
            session_otp: user.session_otp.clone(),
            totp_secret: user.totp_secret.clone(),
            totp_enabled: user.totp_enabled,
            created_by: user.created_by,
            updated_by: user.updated_by,
            created_at: user.created_at,
            updated_at: user.updated_at,
            deleted_at: user.deleted_at,
            avatar_url: user.avatar_url.clone(),
        }
    }
}

impl From<AuthUserCacheEntry> for User {
    fn from(entry: AuthUserCacheEntry) -> Self {
        Self {
            id: entry.id,
            email: entry.email,
            username: entry.username,
            password_hash: entry.password_hash,
            is_active: entry.is_active,
            first_name: entry.first_name,
            last_name: entry.last_name,
            phone_number: entry.phone_number,
            role: entry.role,
            credit_limit: entry.credit_limit,
            session_otp_id: entry.session_otp_id,
            session_otp: entry.session_otp,
            totp_secret: entry.totp_secret,
            totp_enabled: entry.totp_enabled,
            created_by: entry.created_by,
            updated_by: entry.updated_by,
            created_at: entry.created_at,
            updated_at: entry.updated_at,
            deleted_at: entry.deleted_at,
            avatar_url: entry.avatar_url,
        }
    }
}

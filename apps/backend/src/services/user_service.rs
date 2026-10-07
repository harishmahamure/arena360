use crate::dto::{
    JwtUserClaims, KioskRegisterDto, KioskRegisterResponseDto, PaginationResult, RegisterDto,
    RegisterResponseDto,
};
use crate::error::AppError;
use crate::models::{UpdateUserDto, User, UserFilterDto};
use crate::repositories::{
    StaffProjectionResult, TenantCreatePlayer, TenantStaffProjection, TenantUserRepository,
};
use crate::tenancy::TenantDb;
use crate::validation::{
    normalize_phone_digits, trim_optional_string, trim_secret, validate_username,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Default)]
pub struct UserService;
impl UserService {
    pub fn new() -> Self {
        Self
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
}

use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::{JwtUserClaims, PaginationResult, RegisterDto, RegisterResponseDto};
use crate::error::AppError;
use crate::models::{UpdateUserDto, User, UserFilterDto};
use crate::repositories::{CreatePlayerParams, UserRepository};

pub struct UserService {
    repo: UserRepository,
}

impl UserService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: UserRepository::new(pool),
        }
    }

    pub async fn list(&self, filters: UserFilterDto) -> Result<PaginationResult<User>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<User, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("User with ID {id} not found")))
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: UpdateUserDto,
        actor_id: Option<Uuid>,
    ) -> Result<User, AppError> {
        if let Some(username) = &dto.username {
            if self.repo.username_exists(username, Some(id)).await? {
                return Err(AppError::Conflict(format!(
                    "User with username '{username}' already exists"
                )));
            }
        }
        self.repo.update(id, &dto, actor_id).await
    }

    pub async fn register(
        &self,
        dto: RegisterDto,
        claims: &JwtUserClaims,
    ) -> Result<RegisterResponseDto, AppError> {
        if self.repo.username_exists(&dto.username, None).await? {
            return Err(AppError::Conflict(format!(
                "User with username '{}' already exists",
                dto.username
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

        let password_hash = bcrypt::hash(&dto.password, bcrypt::DEFAULT_COST)
            .map_err(|e| AppError::Internal(format!("Failed to hash password: {e}")))?;

        let actor_id = claims.user_id_uuid();

        self.repo
            .create_player(CreatePlayerParams {
                email: dto.email.as_deref(),
                username: &dto.username,
                password_hash: &password_hash,
                first_name: dto.firstName.as_deref(),
                last_name: dto.lastName.as_deref(),
                role,
                actor_id,
            })
            .await?;

        Ok(RegisterResponseDto {
            message: "Created successfully".to_string(),
        })
    }
}

use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::{PaginationResult, RegisterDto, RegisterResponseDto};
use crate::error::AppError;
use crate::models::{UpdateUserDto, User, UserFilterDto};
use crate::repositories::UserRepository;

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

    pub async fn update(&self, id: Uuid, dto: UpdateUserDto) -> Result<User, AppError> {
        if let Some(username) = &dto.username {
            if self.repo.username_exists(username, Some(id)).await? {
                return Err(AppError::Conflict(format!(
                    "User with username '{username}' already exists"
                )));
            }
        }
        self.repo.update(id, &dto).await
    }

    pub async fn register(&self, dto: RegisterDto) -> Result<RegisterResponseDto, AppError> {
        if self.repo.username_exists(&dto.username, None).await? {
            return Err(AppError::Conflict(format!(
                "User with username '{}' already exists",
                dto.username
            )));
        }

        let password_hash = bcrypt::hash(&dto.password, bcrypt::DEFAULT_COST)
            .map_err(|e| AppError::Internal(format!("Failed to hash password: {e}")))?;

        self.repo
            .create_player(
                dto.email.as_deref(),
                &dto.username,
                &password_hash,
                dto.firstName.as_deref(),
                dto.lastName.as_deref(),
            )
            .await?;

        Ok(RegisterResponseDto {
            message: "Created successfully".to_string(),
        })
    }
}

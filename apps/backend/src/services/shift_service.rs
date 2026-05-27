use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{ClockInDto, ClockOutDto, Shift, ShiftFilterDto};
use crate::repositories::ShiftRepository;

#[derive(Clone)]
pub struct ShiftService {
    repo: ShiftRepository,
}

impl ShiftService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: ShiftRepository::new(pool),
        }
    }

    pub async fn clock_in(
        &self,
        user_id: Uuid,
        dto: ClockInDto,
        actor_id: Uuid,
    ) -> Result<Shift, AppError> {
        if let Some(active) = self.repo.find_active_by_user(user_id).await? {
            return Err(AppError::Conflict(format!(
                "User already has an active shift (ID: {}). Clock out first.",
                active.id
            )));
        }
        self.repo.create(user_id, dto.notes, actor_id).await
    }

    pub async fn clock_out(
        &self,
        user_id: Uuid,
        dto: ClockOutDto,
        actor_id: Uuid,
    ) -> Result<Shift, AppError> {
        let active = self
            .repo
            .find_active_by_user(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("No active shift found for user".to_string()))?;

        self.repo.close(active.id, dto.notes, actor_id).await
    }

    pub async fn get_active(&self, user_id: Uuid) -> Result<Option<Shift>, AppError> {
        self.repo.find_active_by_user(user_id).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Shift, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Shift with ID {id} not found")))
    }

    pub async fn list(&self, filters: ShiftFilterDto) -> Result<PaginationResult<Shift>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn force_close(&self, id: Uuid, actor_id: Uuid) -> Result<Shift, AppError> {
        self.repo.force_close(id, actor_id).await
    }
}

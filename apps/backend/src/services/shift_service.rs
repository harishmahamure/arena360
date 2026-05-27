use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{ClockInDto, ClockOutDto, CloseCashRegisterDto, Shift, ShiftFilterDto};
use crate::repositories::ShiftRepository;
use crate::services::CashRegisterService;

pub struct ShiftService {
    repo: ShiftRepository,
    cash_registers: Option<Arc<CashRegisterService>>,
}

impl ShiftService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: ShiftRepository::new(pool),
            cash_registers: None,
        }
    }

    pub fn set_cash_registers(&mut self, cash_registers: Arc<CashRegisterService>) {
        self.cash_registers = Some(cash_registers);
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

        self.auto_close_register(active.id, actor_id).await;

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
        self.auto_close_register(id, actor_id).await;
        self.repo.force_close(id, actor_id).await
    }

    async fn auto_close_register(&self, shift_id: Uuid, actor_id: Uuid) {
        let Some(cash_registers) = &self.cash_registers else {
            return;
        };
        let Ok(register_data) = cash_registers.get_by_shift(shift_id).await else {
            return;
        };
        if register_data.register.status != "open" {
            return;
        }
        let expected = cash_registers
            .get_expected_closing(register_data.register.id)
            .await
            .unwrap_or(register_data.register.opening_balance);
        let _ = cash_registers
            .close(
                register_data.register.id,
                CloseCashRegisterDto {
                    closing_balance: expected,
                    closing_denominations: None,
                    notes: Some("Auto-closed on shift end".to_string()),
                },
                actor_id,
            )
            .await;
    }
}

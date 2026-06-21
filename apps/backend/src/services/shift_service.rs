use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{ClockInDto, ClockOutDto, CloseCashRegisterDto, Shift, ShiftFilterDto};
use crate::repositories::ShiftRepository;
use crate::services::CashRegisterService;
use crate::services::{NotificationService, RecordNotification, Recipients};
use crate::models::activity_kind;

pub struct ShiftService {
    repo: ShiftRepository,
    cash_registers: Option<Arc<CashRegisterService>>,
    notifications: Option<NotificationService>,
}

impl ShiftService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: ShiftRepository::new(pool),
            cash_registers: None,
            notifications: None,
        }
    }

    pub fn set_cash_registers(&mut self, cash_registers: Arc<CashRegisterService>) {
        self.cash_registers = Some(cash_registers);
    }

    pub fn set_notifications(&mut self, notifications: NotificationService) {
        self.notifications = Some(notifications);
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
        let notes = dto.notes.clone();
        let shift = self.repo.create(user_id, notes.clone(), actor_id).await?;
        if let Some(ref notifications) = self.notifications {
            let _ = notifications
                .record(RecordNotification {
                    kind: activity_kind::SHIFT_CLOCK_IN.to_string(),
                    title: "Shift started".to_string(),
                    summary: notes,
                    payload: serde_json::json!({ "shiftId": shift.id.to_string() }),
                    actor_user_id: Some(actor_id),
                    entity_type: Some("shift".to_string()),
                    entity_id: Some(shift.id),
                    recipients: Recipients::Users(vec![user_id]),
                })
                .await;
        }
        Ok(shift)
    }

    /// Starts a shift on staff login, recovering from a stale active shift when its register is already closed.
    pub async fn ensure_shift_for_staff_login(
        &self,
        user_id: Uuid,
        actor_id: Uuid,
    ) -> Result<Shift, AppError> {
        let login_notes = Some("Auto-started on login".to_string());
        match self
            .clock_in(
                user_id,
                ClockInDto {
                    notes: login_notes.clone(),
                },
                actor_id,
            )
            .await
        {
            Ok(shift) => Ok(shift),
            Err(AppError::Conflict(_)) => {
                let active = self.get_active(user_id).await?.ok_or_else(|| {
                    AppError::Internal("Active shift conflict without active shift".to_string())
                })?;

                let Some(cash_registers) = &self.cash_registers else {
                    return Ok(active);
                };

                if let Some(register) = cash_registers.find_register_by_shift(active.id).await? {
                    if register.status == "closed" {
                        self.clock_out(
                            user_id,
                            ClockOutDto {
                                notes: Some("Auto-closed stale shift on login".to_string()),
                            },
                            actor_id,
                        )
                        .await?;
                        return self
                            .clock_in(
                                user_id,
                                ClockInDto { notes: login_notes },
                                actor_id,
                            )
                            .await;
                    }
                }

                Ok(active)
            }
            Err(error) => Err(error),
        }
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

        let shift = self.repo.close(active.id, dto.notes.clone(), actor_id).await?;
        if let Some(ref notifications) = self.notifications {
            let _ = notifications
                .record(RecordNotification {
                    kind: activity_kind::SHIFT_CLOCK_OUT.to_string(),
                    title: "Shift ended".to_string(),
                    summary: dto.notes.clone(),
                    payload: serde_json::json!({ "shiftId": shift.id.to_string() }),
                    actor_user_id: Some(actor_id),
                    entity_type: Some("shift".to_string()),
                    entity_id: Some(shift.id),
                    recipients: Recipients::Users(vec![user_id]),
                })
                .await;
        }
        Ok(shift)
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

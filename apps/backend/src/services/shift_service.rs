use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::activity_kind;
use crate::models::{
    CashRegister, ClockInDto, ClockOutDto, CloseCashRegisterDto, Shift, ShiftFilterDto,
    ShiftStartResponseDto, StartShiftDto,
};
use crate::repositories::ShiftRepository;
use crate::services::CashRegisterService;
use crate::services::{NotificationService, Recipients, RecordNotification};

pub struct ShiftService {
    repo: ShiftRepository,
    pool: PgPool,
    cash_registers: Option<Arc<CashRegisterService>>,
    notifications: Option<NotificationService>,
}

impl ShiftService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: ShiftRepository::new(pool.clone()),
            pool,
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
        self.assert_user_is_staff(user_id).await?;

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
                .record_activity(RecordNotification {
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

    pub async fn start_confirmed(
        &self,
        user_id: Uuid,
        dto: StartShiftDto,
        actor_id: Uuid,
    ) -> Result<ShiftStartResponseDto, AppError> {
        self.assert_user_is_staff(user_id).await?;
        if dto.opening_balance < 0.0 {
            return Err(AppError::bad_request_code(
                "SHIFT_INVALID_OPENING_BALANCE",
                None,
            ));
        }

        let mut tx = self.pool.begin().await?;
        sqlx::query(r#"SELECT id FROM users WHERE id = $1 FOR UPDATE"#)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

        let active = sqlx::query_as::<_, Shift>(
            r#"SELECT id, "userId" as user_id, "clockIn" as clock_in,
                      "clockOut" as clock_out, notes, status,
                      "createdBy" as created_by, "updatedBy" as updated_by,
                      "createdAt" as created_at, "updatedAt" as updated_at
               FROM shifts WHERE "userId" = $1 AND status = 'active'"#,
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(shift) = active {
            let register = sqlx::query_as::<_, CashRegister>(
                r#"SELECT id, "shiftId" as shift_id, "openedBy" as opened_by,
                          "closedBy" as closed_by, "openingBalance"::float8 as opening_balance,
                          "openingDenominations" as opening_denominations,
                          "closingBalance"::float8 as closing_balance,
                          "closingDenominations" as closing_denominations,
                          "expectedClosing"::float8 as expected_closing, variance::float8 as variance,
                          status, notes, "reconciledBy" as reconciled_by,
                          "reconciledAt" as reconciled_at, "reconciliationNotes" as reconciliation_notes,
                          "createdBy" as created_by, "updatedBy" as updated_by,
                          "createdAt" as created_at, "updatedAt" as updated_at,
                          NULL::float8 as total_cash_in, NULL::float8 as total_cash_out,
                          NULL::float8 as total_deposited
                   FROM cash_registers WHERE "shiftId" = $1 AND status = 'open'"#,
            )
            .bind(shift.id)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some(register) = register {
                tx.commit().await?;
                return Ok(ShiftStartResponseDto {
                    resumed: true,
                    shift,
                    cash_register: register,
                });
            }

            sqlx::query(
                r#"UPDATE shifts SET status='completed', "clockOut"=NOW(), "updatedBy"=$2, "updatedAt"=NOW() WHERE id=$1"#,
            )
            .bind(shift.id)
            .bind(actor_id)
            .execute(&mut *tx)
            .await?;
        }

        let shift = sqlx::query_as::<_, Shift>(
            r#"INSERT INTO shifts
                  (id, "userId", "clockIn", notes, status, "createdBy", "updatedBy", "createdAt", "updatedAt")
               VALUES (gen_random_uuid(), $1, NOW(), $2, 'active', $3, $3, NOW(), NOW())
               RETURNING id, "userId" as user_id, "clockIn" as clock_in,
                         "clockOut" as clock_out, notes, status,
                         "createdBy" as created_by, "updatedBy" as updated_by,
                         "createdAt" as created_at, "updatedAt" as updated_at"#,
        )
        .bind(user_id)
        .bind(dto.notes.clone())
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await?;

        let register = sqlx::query_as::<_, CashRegister>(
            r#"INSERT INTO cash_registers
                  (id, "shiftId", "openedBy", "openingBalance", "openingDenominations",
                   notes, status, "createdBy", "updatedBy", "createdAt", "updatedAt")
               VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, 'open', $2, $2, NOW(), NOW())
               RETURNING id, "shiftId" as shift_id, "openedBy" as opened_by,
                         "closedBy" as closed_by, "openingBalance"::float8 as opening_balance,
                         "openingDenominations" as opening_denominations,
                         "closingBalance"::float8 as closing_balance,
                         "closingDenominations" as closing_denominations,
                         "expectedClosing"::float8 as expected_closing, variance::float8 as variance,
                         status, notes, "reconciledBy" as reconciled_by,
                         "reconciledAt" as reconciled_at, "reconciliationNotes" as reconciliation_notes,
                         "createdBy" as created_by, "updatedBy" as updated_by,
                         "createdAt" as created_at, "updatedAt" as updated_at,
                         NULL::float8 as total_cash_in, NULL::float8 as total_cash_out,
                         NULL::float8 as total_deposited"#,
        )
        .bind(shift.id)
        .bind(actor_id)
        .bind(dto.opening_balance)
        .bind(dto.opening_denominations)
        .bind(dto.notes)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        if let Some(ref notifications) = self.notifications {
            let _ = notifications
                .record_activity(RecordNotification {
                    kind: activity_kind::SHIFT_CLOCK_IN.to_string(),
                    title: "Shift started".to_string(),
                    summary: shift.notes.clone(),
                    payload: serde_json::json!({ "shiftId": shift.id.to_string() }),
                    actor_user_id: Some(actor_id),
                    entity_type: Some("shift".to_string()),
                    entity_id: Some(shift.id),
                    recipients: Recipients::Users(vec![user_id]),
                })
                .await;
        }

        Ok(ShiftStartResponseDto {
            resumed: false,
            shift,
            cash_register: register,
        })
    }

    /// Starts a shift on staff login, recovering from a stale active shift when its register is already closed.
    pub async fn ensure_shift_for_staff_login(
        &self,
        user_id: Uuid,
        actor_id: Uuid,
    ) -> Result<Shift, AppError> {
        self.assert_user_is_staff(user_id).await?;

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
                            .clock_in(user_id, ClockInDto { notes: login_notes }, actor_id)
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

        let shift = self
            .repo
            .close(active.id, dto.notes.clone(), actor_id)
            .await?;
        if let Some(ref notifications) = self.notifications {
            let _ = notifications
                .record_activity(RecordNotification {
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

    async fn assert_user_is_staff(&self, user_id: Uuid) -> Result<(), AppError> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as(r#"SELECT role FROM users WHERE id = $1 AND "deletedAt" IS NULL"#)
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;

        match row.and_then(|(role,)| role).as_deref() {
            Some("staff") => Ok(()),
            Some("admin") => Err(AppError::Forbidden("Admins cannot open shifts".to_string())),
            _ => Err(AppError::Forbidden(
                "Only staff can open shifts".to_string(),
            )),
        }
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

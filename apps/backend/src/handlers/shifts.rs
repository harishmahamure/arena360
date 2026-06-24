use axum::extract::{Path, Query, State};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult, PaginationResult};
use crate::middleware::{AdminOrStaff, AdminUser, StaffUser};
use crate::models::{
    ClockInDto, ClockOutDto, CloseCashRegisterDto, InitiateDepositDto, Shift, ShiftCloseDto,
    ShiftCloseResponseDto, ShiftFilterDto, ShiftHandoverDto, ShiftHandoverResponseDto,
};
use crate::openapi::responses::{
    ErrorEnvelope, ShiftCloseResponseEnvelope, ShiftEnvelope, ShiftHandoverResponseEnvelope,
    ShiftPaginationEnvelope,
};

#[utoipa::path(
    post,
    path = "/shifts/clock-in",
    request_body = ClockInDto,
    responses(
        (status = 201, description = "Shift started", body = ShiftEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 409, description = "Already clocked in", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn clock_in(
    StaffUser(claims): StaffUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<ClockInDto>,
) -> ApiResult<Shift> {
    let user_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID in token".to_string()))?;
    let shift = state.shifts.clock_in(user_id, dto, user_id).await?;
    state
        .cash_registers
        .carry_forward_balance(user_id, shift.id, user_id)
        .await?;
    created(shift)
}

#[utoipa::path(
    patch,
    path = "/shifts/clock-out",
    request_body = ClockOutDto,
    responses(
        (status = 200, description = "Shift ended", body = ShiftEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "No active shift", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn clock_out(
    StaffUser(claims): StaffUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<ClockOutDto>,
) -> ApiResult<Shift> {
    let user_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID in token".to_string()))?;
    let shift = state.shifts.clock_out(user_id, dto, user_id).await?;
    ok(shift)
}

#[utoipa::path(
    get,
    path = "/shifts/active",
    responses(
        (status = 200, description = "Active shift or null", body = ShiftEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn get_active_shift(
    StaffUser(claims): StaffUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Option<Shift>> {
    let user_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID in token".to_string()))?;
    let shift = state.shifts.get_active(user_id).await?;
    ok(shift)
}

#[utoipa::path(
    get,
    path = "/shifts",
    params(ShiftFilterDto),
    responses(
        (status = 200, description = "List shifts", body = ShiftPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn list_shifts(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(mut filters): Query<ShiftFilterDto>,
) -> ApiResult<PaginationResult<Shift>> {
    if !claims.is_admin() {
        let user_id: Uuid = claims.userId.parse().map_err(|_| {
            crate::error::AppError::BadRequest("Invalid user ID in token".to_string())
        })?;
        filters.user_id = Some(user_id);
    }
    let result = state.shifts.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/shifts/{id}",
    params(
        ("id" = Uuid, Path, description = "Shift ID"),
    ),
    responses(
        (status = 200, description = "Get shift", body = ShiftEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn get_shift(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Shift> {
    let shift = state.shifts.get_by_id(id).await?;

    if !claims.is_admin() {
        let user_id: Uuid = claims.userId.parse().map_err(|_| {
            crate::error::AppError::BadRequest("Invalid user ID in token".to_string())
        })?;
        if shift.user_id != user_id {
            return Err(crate::error::AppError::Forbidden(
                "Cannot view another user's shift".to_string(),
            ));
        }
    }

    ok(shift)
}

#[utoipa::path(
    patch,
    path = "/shifts/{id}/force-close",
    params(
        ("id" = Uuid, Path, description = "Shift ID"),
    ),
    responses(
        (status = 200, description = "Shift force-closed", body = ShiftEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn force_close_shift(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Shift> {
    let actor_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID in token".to_string()))?;
    let shift = state.shifts.force_close(id, actor_id).await?;
    ok(shift)
}

#[utoipa::path(
    post,
    path = "/shifts/handover",
    request_body = ShiftHandoverDto,
    responses(
        (status = 200, description = "Shift handover completed", body = ShiftHandoverResponseEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn handover_shift(
    StaffUser(claims): StaffUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<ShiftHandoverDto>,
) -> ApiResult<ShiftHandoverResponseDto> {
    let staff_a_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID in token".to_string()))?;

    let active_shift = state.shifts.get_active(staff_a_id).await?.ok_or_else(|| {
        crate::error::AppError::NotFound("No active shift found for current user".to_string())
    })?;

    let validator = state
        .auth
        .authenticate_staff_with_totp(
            &dto.validator_username,
            &dto.validator_password,
            &dto.validator_totp,
        )
        .await?;

    if validator.id == staff_a_id {
        return Err(crate::error::AppError::BadRequest(
            "Handover validator must be a different staff member".to_string(),
        ));
    }

    let register_with_entries = state
        .cash_registers
        .get_by_shift(active_shift.id)
        .await
        .ok();

    let mut deposit = None;
    if let (Some(ref register_data), Some(deposit_dto)) =
        (&register_with_entries, dto.deposit.as_ref())
    {
        if register_data.register.status == "open" {
            deposit = Some(
                state
                    .cash_deposits
                    .initiate(
                        InitiateDepositDto {
                            cash_register_id: register_data.register.id,
                            shift_id: active_shift.id,
                            amount: deposit_dto.amount,
                            denominations: deposit_dto.denominations.clone(),
                            notes: deposit_dto.notes.clone(),
                        },
                        staff_a_id,
                    )
                    .await?,
            );
        }
    }

    let closed_register = if let Some(ref register_data) = register_with_entries {
        if register_data.register.status == "open" {
            Some(
                state
                    .cash_registers
                    .close(
                        register_data.register.id,
                        CloseCashRegisterDto {
                            closing_balance: dto.closing_balance,
                            closing_denominations: dto.closing_denominations.clone(),
                            notes: dto.notes.clone(),
                        },
                        validator.id,
                    )
                    .await?,
            )
        } else {
            let stored_closing = register_data.register.closing_balance.unwrap_or(0.0);
            if (stored_closing - dto.closing_balance).abs() > 0.01 {
                return Err(crate::error::AppError::BadRequest(
                    "Cash register is already closed with a different balance. Contact an admin."
                        .to_string(),
                ));
            }
            Some(register_data.register.clone())
        }
    } else {
        None
    };

    let closed_shift = state
        .shifts
        .clock_out(
            staff_a_id,
            ClockOutDto {
                notes: dto.notes.clone(),
            },
            validator.id,
        )
        .await?;

    let new_shift = state
        .shifts
        .clock_in(
            validator.id,
            ClockInDto {
                notes: Some("Auto-started on handover".to_string()),
            },
            validator.id,
        )
        .await?;

    if let Some(ref closed_reg) = closed_register {
        state
            .cash_registers
            .apply_carry_forward_from_register(new_shift.id, validator.id, closed_reg)
            .await?;
    } else {
        state
            .cash_registers
            .ensure_open_for_shift(new_shift.id, validator.id, 0.0)
            .await?;
    }

    let mut auth_response = state.auth.issue_auth_response(&validator)?;
    auth_response.shiftId = Some(new_shift.id.to_string());

    let _ = state
        .notifications
        .record(crate::services::RecordNotification {
            kind: crate::models::activity_kind::SHIFT_HANDOVER.to_string(),
            title: "Shift handover completed".to_string(),
            summary: dto.notes.clone(),
            payload: serde_json::json!({
                "closedShiftId": closed_shift.id.to_string(),
                "newShiftId": new_shift.id.to_string(),
                "fromStaffId": staff_a_id.to_string(),
                "toStaffId": validator.id.to_string(),
            }),
            actor_user_id: Some(staff_a_id),
            entity_type: Some("shift".to_string()),
            entity_id: Some(closed_shift.id),
            recipients: crate::services::Recipients::Users(vec![staff_a_id, validator.id]),
        })
        .await;

    ok(ShiftHandoverResponseDto {
        closedShift: closed_shift,
        cashRegister: closed_register,
        deposit,
        newAccessToken: auth_response.accessToken,
        newUser: auth_response.user,
        newShiftId: new_shift.id.to_string(),
    })
}

#[utoipa::path(
    post,
    path = "/shifts/close",
    request_body = ShiftCloseDto,
    responses(
        (status = 200, description = "Shift closed", body = ShiftCloseResponseEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "No active shift", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn close_shift(
    StaffUser(claims): StaffUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<ShiftCloseDto>,
) -> ApiResult<ShiftCloseResponseDto> {
    let staff_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID in token".to_string()))?;

    let active_shift = state.shifts.get_active(staff_id).await?.ok_or_else(|| {
        crate::error::AppError::NotFound("No active shift found for current user".to_string())
    })?;

    let register_with_entries = state
        .cash_registers
        .get_by_shift(active_shift.id)
        .await
        .ok();

    let mut deposit = None;
    if let (Some(ref register_data), Some(deposit_dto)) =
        (&register_with_entries, dto.deposit.as_ref())
    {
        if register_data.register.status == "open" {
            deposit = Some(
                state
                    .cash_deposits
                    .initiate(
                        InitiateDepositDto {
                            cash_register_id: register_data.register.id,
                            shift_id: active_shift.id,
                            amount: deposit_dto.amount,
                            denominations: deposit_dto.denominations.clone(),
                            notes: deposit_dto.notes.clone(),
                        },
                        staff_id,
                    )
                    .await?,
            );
        }
    }

    let closed_register = if let Some(ref register_data) = register_with_entries {
        if register_data.register.status == "open" {
            Some(
                state
                    .cash_registers
                    .close(
                        register_data.register.id,
                        CloseCashRegisterDto {
                            closing_balance: dto.closing_balance,
                            closing_denominations: dto.closing_denominations.clone(),
                            notes: dto.notes.clone(),
                        },
                        staff_id,
                    )
                    .await?,
            )
        } else {
            Some(register_data.register.clone())
        }
    } else {
        None
    };

    let closed_shift = state
        .shifts
        .clock_out(
            staff_id,
            ClockOutDto {
                notes: dto.notes.clone(),
            },
            staff_id,
        )
        .await?;

    ok(ShiftCloseResponseDto {
        closedShift: closed_shift,
        cashRegister: closed_register,
        deposit,
    })
}

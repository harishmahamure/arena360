use axum::extract::{Path, Query, State};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult, PaginationResult};
use crate::middleware::{AdminOrStaff, AdminUser, StaffUser};
use crate::models::{
    ClockInDto, ClockOutDto, Shift, ShiftCloseDto, ShiftCloseResponseDto, ShiftFilterDto,
    ShiftHandoverDto, ShiftHandoverResponseDto, ShiftStartContextDto, ShiftStartResponseDto,
    StartShiftDto,
};
use crate::openapi::responses::{
    ErrorEnvelope, ShiftCloseResponseEnvelope, ShiftEnvelope, ShiftHandoverResponseEnvelope,
    ShiftPaginationEnvelope, ShiftStartContextEnvelope, ShiftStartResponseEnvelope,
};
use crate::repositories::{
    TenantCashRegisterRepository, TenantSettingsRepository, TenantShiftRepository,
};

#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ShiftVenueQuery {
    pub location_id: Option<Uuid>,
}

async fn shift_venue(
    state: &AppState,
    claims: &crate::dto::JwtUserClaims,
    shift_id: Uuid,
) -> Result<Uuid, crate::error::AppError> {
    let db = state.business_db(claims).await?;
    let id: String = sqlx::query_scalar("SELECT location_id FROM shifts WHERE id=?")
        .bind(shift_id.to_string())
        .fetch_optional(&db.read_pool()?)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("Shift not found".into()))?;
    id.parse()
        .map_err(|_| crate::error::AppError::Internal("Invalid stored location".into()))
}

async fn require_shift_venue(
    state: &AppState,
    claims: &crate::dto::JwtUserClaims,
    venue: Uuid,
    permission: &str,
) -> Result<(), crate::error::AppError> {
    let db = state.business_db(claims).await?;
    let user = claims
        .user_id_uuid()
        .ok_or_else(|| crate::error::AppError::Unauthorized("Invalid user identity".into()))?;
    TenantSettingsRepository::new(db.clone())
        .ensure_location_permission(db.tenant_id(), venue, user, permission)
        .await
}

async fn selected_venue(
    state: &AppState,
    claims: &crate::dto::JwtUserClaims,
    requested: Option<Uuid>,
) -> Result<Uuid, crate::error::AppError> {
    let scope = crate::access::scope::LocationScope::resolve_tenant(
        state.business_db(claims).await?,
        claims,
        "shifts:write",
        requested,
    )
    .await?;
    if requested.is_none() && scope.locations.len() != 1 {
        return Err(crate::error::AppError::BadRequest(
            "Select a location before starting a shift".into(),
        ));
    }
    scope.first()
}

#[utoipa::path(
    get,
    path = "/shifts/start-context",
    responses(
        (status = 200, description = "Resumable shift or carry-forward opening float", body = ShiftStartContextEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Staff only", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn start_context(
    StaffUser(claims): StaffUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<ShiftVenueQuery>,
) -> ApiResult<ShiftStartContextDto> {
    let venue_id = selected_venue(&state, &claims, query.location_id).await?;
    require_shift_venue(&state, &claims, venue_id, "shifts:read").await?;
    let user_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID in token".to_string()))?;
    if let Some(shift) = TenantShiftRepository::new(state.business_db(&claims).await?)
        .find_active_by_user(user_id)
        .await?
    {
        if shift_venue(&state, &claims, shift.id).await? != venue_id {
            return Err(crate::error::AppError::Conflict(
                "An active shift belongs to another location".into(),
            ));
        }
        if let Ok(register_with_entries) =
            TenantCashRegisterRepository::new(state.business_db(&claims).await?)
                .get_by_shift(shift.id)
                .await
        {
            let register = register_with_entries.register;
            if register.status == "open" {
                return ok(ShiftStartContextDto {
                    mode: "resume".to_string(),
                    suggested_opening_balance: register.opening_balance,
                    shift: Some(shift),
                    cash_register: Some(register),
                });
            }
        }
    }
    let suggested = TenantCashRegisterRepository::new(state.business_db(&claims).await?)
        .preview_carry_forward_balance_for(venue_id)
        .await?;
    ok(ShiftStartContextDto {
        mode: "start".to_string(),
        shift: None,
        cash_register: None,
        suggested_opening_balance: suggested,
    })
}

#[utoipa::path(
    post,
    path = "/shifts/start",
    request_body = StartShiftDto,
    responses(
        (status = 200, description = "Shift and register started or resumed atomically", body = ShiftStartResponseEnvelope),
        (status = 400, description = "Invalid opening balance", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Staff only", body = ErrorEnvelope),
        (status = 409, description = "Concurrent start conflict", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "shifts"
)]
pub async fn start_shift(
    StaffUser(claims): StaffUser,
    State(state): State<Arc<AppState>>,
    Json(mut dto): Json<StartShiftDto>,
) -> ApiResult<ShiftStartResponseDto> {
    require_shift_venue(
        &state,
        &claims,
        selected_venue(&state, &claims, dto.venue_location_id).await?,
        "shifts:write",
    )
    .await?;
    dto.venue_location_id = Some(selected_venue(&state, &claims, dto.venue_location_id).await?);
    let user_id: Uuid = claims
        .userId
        .parse()
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID in token".to_string()))?;
    ok(
        TenantShiftRepository::new(state.business_db(&claims).await?)
            .start_confirmed(user_id, dto, user_id)
            .await?,
    )
}

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
    let user = claims
        .user_id_uuid()
        .ok_or_else(|| crate::error::AppError::Unauthorized("Invalid user identity".into()))?;
    let venue = selected_venue(&state, &claims, None).await?;
    let db = state.business_db(&claims).await?;
    let opening = TenantCashRegisterRepository::new(db.clone())
        .preview_carry_forward_balance_for(venue)
        .await?;
    let started = TenantShiftRepository::new(db)
        .start_confirmed(
            user,
            StartShiftDto {
                opening_balance: opening,
                opening_denominations: None,
                notes: dto.notes,
                venue_location_id: Some(venue),
            },
            user,
        )
        .await?;
    created(started.shift)
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
    let user = claims
        .user_id_uuid()
        .ok_or_else(|| crate::error::AppError::Unauthorized("Invalid user identity".into()))?;
    let repo = TenantShiftRepository::new(state.business_db(&claims).await?);
    let active = repo
        .find_active_by_user(user)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("No active shift found".into()))?;
    require_shift_venue(
        &state,
        &claims,
        shift_venue(&state, &claims, active.id).await?,
        "shifts:write",
    )
    .await?;
    ok(repo.close(active.id, dto.notes, user).await?)
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
    let shift = TenantShiftRepository::new(state.business_db(&claims).await?)
        .find_active_by_user(user_id)
        .await?;
    if let Some(ref active) = shift {
        require_shift_venue(
            &state,
            &claims,
            shift_venue(&state, &claims, active.id).await?,
            "shifts:read",
        )
        .await?;
    }
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
    if !crate::access::has(&claims, "finance:read") {
        let user_id: Uuid = claims.userId.parse().map_err(|_| {
            crate::error::AppError::BadRequest("Invalid user ID in token".to_string())
        })?;
        filters.user_id = Some(user_id);
    }
    let result = TenantShiftRepository::new(state.business_db(&claims).await?)
        .list(&filters)
        .await?;
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
    let shift = TenantShiftRepository::new(state.business_db(&claims).await?)
        .find_by_id(id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("Shift not found".into()))?;

    if !crate::access::has(&claims, "finance:read") {
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
    let shift = TenantShiftRepository::new(state.business_db(&claims).await?)
        .force_close(id, actor_id)
        .await?;
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
    let user = claims
        .user_id_uuid()
        .ok_or_else(|| crate::error::AppError::Unauthorized("Invalid user identity".into()))?;
    let db = state.business_db(&claims).await?;
    let repo = TenantShiftRepository::new(db.clone());
    let active = repo
        .find_active_by_user(user)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("No active shift found".into()))?;
    let venue = shift_venue(&state, &claims, active.id).await?;
    require_shift_venue(&state, &claims, venue, "shifts:write").await?;
    let validator = state
        .auth
        .authenticate_staff_with_totp(
            &dto.validator_username,
            &dto.validator_password,
            &dto.validator_totp,
        )
        .await?;
    TenantSettingsRepository::new(db.clone())
        .ensure_location_permission(db.tenant_id(), venue, validator.id, "shifts:write")
        .await?;
    let auth = state.auth.issue_auth_response(&validator).await?;
    // The token must select the same tenant as the handover before any cash is moved.
    let token = crate::middleware::auth::decode_token(&state, &auth.accessToken)?;
    if token.tenantId != claims.tenantId {
        return Err(crate::error::AppError::Conflict(
            "Validator must select this tenant before handover".into(),
        ));
    }
    let (closed, new_shift, _) = repo
        .handover(
            active.id,
            validator.id,
            ShiftCloseDto {
                closing_balance: dto.closing_balance,
                closing_denominations: dto.closing_denominations,
                notes: dto.notes,
                deposit: dto.deposit,
            },
            validator.id,
        )
        .await?;
    ok(ShiftHandoverResponseDto {
        closedShift: closed.closedShift,
        cashRegister: closed.cashRegister,
        deposit: closed.deposit,
        newAccessToken: auth.accessToken,
        newUser: auth.user,
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
    let user = claims
        .user_id_uuid()
        .ok_or_else(|| crate::error::AppError::Unauthorized("Invalid user identity".into()))?;
    let repo = TenantShiftRepository::new(state.business_db(&claims).await?);
    let active = repo
        .find_active_by_user(user)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("No active shift found".into()))?;
    require_shift_venue(
        &state,
        &claims,
        shift_venue(&state, &claims, active.id).await?,
        "shifts:write",
    )
    .await?;
    ok(repo.close_with_register(active.id, dto, user).await?)
}

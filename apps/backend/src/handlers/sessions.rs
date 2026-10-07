use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::middleware::{require_staff_for_counter, AdminOrStaff, AuthUser};
use crate::models::{
    CreateSessionDto, EndSessionDto, SessionFilterDto, UsageSession, UsageSessionResponse,
};
use crate::openapi::responses::{
    ErrorEnvelope, SessionEnvelope, SessionFlatEnvelope, SessionPaginationEnvelope,
};

#[utoipa::path(
    get,
    path = "/sessions",
    responses(
        (status = 200, description = "List sessions", body = SessionPaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "sessions"
)]
pub async fn list_sessions(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<SessionFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<UsageSessionResponse>> {
    let db = state.business_db(&claims).await?;
    let timezone = db.timezone().await?;
    let scope = crate::access::scope::LocationScope::resolve_tenant(
        db.clone(),
        &claims,
        "sessions:read",
        None,
    )
    .await?;
    let result = state
        .sessions
        .list_tenant(db, filters, scope.locations, timezone)
        .await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/sessions/active",
    responses(
        (status = 200, description = "List active sessions", body = SessionPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "sessions"
)]
pub async fn list_active_sessions(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<crate::dto::PaginationResult<UsageSessionResponse>> {
    let db = state.business_db(&claims).await?;
    let timezone = db.timezone().await?;
    let scope = crate::access::scope::LocationScope::resolve_tenant(
        db.clone(),
        &claims,
        "sessions:read",
        None,
    )
    .await?;
    let result = state
        .sessions
        .list_tenant(
            db,
            SessionFilterDto {
                is_active: Some(1),
                ..Default::default()
            },
            scope.locations,
            timezone,
        )
        .await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/sessions/{id}",
    params(
        ("id" = Uuid, Path, description = "Session ID"),
    ),
    responses(
        (status = 200, description = "Get session", body = SessionEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "sessions"
)]
pub async fn get_session(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<UsageSessionResponse> {
    let db = state.business_db(&claims).await?;
    let timezone = db.timezone().await?;
    let session = state.sessions.get_by_id_tenant(db, id, timezone).await?;
    ok(session)
}

#[utoipa::path(
    post,
    path = "/sessions",
    request_body = CreateSessionDto,
    responses(
        (status = 201, description = "Create session", body = SessionFlatEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "sessions"
)]
pub async fn create_session(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(mut dto): Json<CreateSessionDto>,
) -> ApiResult<UsageSession> {
    let user_id = claims.user_id_uuid().ok_or_else(|| {
        crate::error::AppError::BadRequest("Invalid user ID in token".to_string())
    })?;

    require_staff_for_counter(&claims)?;

    // Enforce active shift
    let active_shift =
        crate::repositories::TenantShiftRepository::new(state.business_db(&claims).await?)
            .find_active_by_user(user_id)
            .await?
            .ok_or_else(|| {
                crate::error::AppError::BadRequest(
                    "No active shift found for current user".to_string(),
                )
            })?;

    dto.shift_id = Some(active_shift.id);

    let db = state.business_db(&claims).await?;
    let balance = state
        .balances
        .get_raw_tenant(db.clone(), dto.balance_id)
        .await?;
    let session = state
        .sessions
        .start_tenant(db, dto, balance.player_id, Some(user_id))
        .await?;
    created(session)
}

#[utoipa::path(
    patch,
    path = "/sessions/{id}/end",
    params(
        ("id" = Uuid, Path, description = "Session ID"),
    ),
    request_body = EndSessionDto,
    responses(
        (status = 200, description = "End session", body = SessionFlatEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "sessions"
)]
pub async fn end_session(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<EndSessionDto>,
) -> ApiResult<UsageSession> {
    let actor_id = claims.user_id_uuid().ok_or_else(|| {
        crate::error::AppError::BadRequest("Invalid user ID in token".to_string())
    })?;

    if claims.is_staff() {
        state
            .auth
            .verify_optional_staff_totp(actor_id, dto.staff_totp.as_deref())
            .await?;
    }
    let session = state
        .sessions
        .end_tenant(state.business_db(&claims).await?, id, dto, Some(actor_id))
        .await?;

    ok(session)
}

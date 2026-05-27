use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::models::{CreateSessionDto, EndSessionDto, SessionFilterDto, UsageSession, UsageSessionResponse};
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
    State(state): State<Arc<AppState>>,
    Query(filters): Query<SessionFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<UsageSessionResponse>> {
    let result = state.sessions.list(filters).await?;
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
    State(state): State<Arc<AppState>>,
) -> ApiResult<crate::dto::PaginationResult<UsageSessionResponse>> {
    let result = state.sessions.list_active().await?;
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
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<UsageSessionResponse> {
    let session = state.sessions.get_by_id(id).await?;
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
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateSessionDto>,
) -> ApiResult<UsageSession> {
    let session = state.sessions.start(dto).await?;
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
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<EndSessionDto>,
) -> ApiResult<UsageSession> {
    let session = state.sessions.end(id, dto).await?;
    ok(session)
}

use axum::{
    extract::{Path, Query, State},
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{ok, ApiResult, PaginationResult};
use crate::middleware::AdminOrStaff;
use crate::models::{
    ActivityLog, ActivityLogFilterDto, NotificationFilterDto, NotificationItem, UnreadCountDto,
};
use crate::openapi::responses::{
    ActivityLogPaginationEnvelope, ErrorEnvelope, NotificationPaginationEnvelope,
    UnreadCountEnvelope,
};

#[utoipa::path(
    get,
    path = "/notifications",
    params(NotificationFilterDto),
    responses(
        (status = 200, description = "List notifications", body = NotificationPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "notifications"
)]
pub async fn list_notifications(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<NotificationFilterDto>,
) -> ApiResult<PaginationResult<NotificationItem>> {
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    let result = state.notifications.list_notifications(user_id, filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/notifications/unread-count",
    params(NotificationFilterDto),
    responses(
        (status = 200, description = "Unread notification count", body = UnreadCountEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "notifications"
)]
pub async fn unread_count(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<NotificationFilterDto>,
) -> ApiResult<UnreadCountDto> {
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    let result = state.notifications.unread_count(user_id, filters).await?;
    ok(result)
}

#[utoipa::path(
    patch,
    path = "/notifications/{id}/read",
    params(
        ("id" = Uuid, Path, description = "Notification ID"),
    ),
    responses(
        (status = 200, description = "Marked read", body = UnreadCountEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "notifications"
)]
pub async fn mark_read(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<UnreadCountDto> {
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    let updated = state.notifications.mark_read(id, user_id).await?;
    if !updated {
        return Err(crate::error::AppError::NotFound(format!(
            "Notification {id} not found"
        )));
    }
    let count = state
        .notifications
        .unread_count(user_id, NotificationFilterDto::default())
        .await?;
    ok(count)
}

#[utoipa::path(
    post,
    path = "/notifications/read-all",
    responses(
        (status = 200, description = "All marked read", body = UnreadCountEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "notifications"
)]
pub async fn mark_all_read(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
) -> ApiResult<UnreadCountDto> {
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    let _ = state.notifications.mark_all_read(user_id).await?;
    let count = state
        .notifications
        .unread_count(user_id, NotificationFilterDto::default())
        .await?;
    ok(count)
}

#[utoipa::path(
    get,
    path = "/activity-log",
    responses(
        (status = 200, description = "Activity log", body = ActivityLogPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "notifications"
)]
pub async fn list_activity_log(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<ActivityLogFilterDto>,
) -> ApiResult<PaginationResult<ActivityLog>> {
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    let is_admin = claims.is_admin();
    let result = state
        .notifications
        .list_activity_log(user_id, is_admin, filters)
        .await?;
    ok(result)
}

use axum::extract::{Path, Query, State};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{ok, ApiResult, PaginationResult};
use crate::middleware::{AdminOrStaff, StaffUser};
use crate::models::{
    ActivityLog, ActivityLogFilterDto, NotificationFilterDto, NotificationItem, UnreadCountDto,
};
use crate::openapi::responses::{
    ActivityLogPaginationEnvelope, ErrorEnvelope, NotificationPaginationEnvelope,
    UnreadCountEnvelope,
};
use crate::repositories::TenantNotificationRepository;

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
    StaffUser(claims): StaffUser,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<NotificationFilterDto>,
) -> ApiResult<PaginationResult<NotificationItem>> {
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    let result = TenantNotificationRepository::new(state.business_db(&claims).await?)
        .list_notifications(user_id, &filters)
        .await?;
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
    StaffUser(claims): StaffUser,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<NotificationFilterDto>,
) -> ApiResult<UnreadCountDto> {
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    let result = TenantNotificationRepository::new(state.business_db(&claims).await?)
        .unread_count(user_id, filters.important_only.unwrap_or(false))
        .await?;
    ok(UnreadCountDto { count: result })
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
    StaffUser(claims): StaffUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<UnreadCountDto> {
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    let updated = TenantNotificationRepository::new(state.business_db(&claims).await?)
        .mark_read(id, user_id)
        .await?;
    if !updated {
        return Err(crate::error::AppError::NotFound(format!(
            "Notification {id} not found"
        )));
    }
    let count = TenantNotificationRepository::new(state.business_db(&claims).await?)
        .unread_count(user_id, false)
        .await?;
    ok(UnreadCountDto { count })
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
    StaffUser(claims): StaffUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<UnreadCountDto> {
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    let _ = TenantNotificationRepository::new(state.business_db(&claims).await?)
        .mark_all_read(user_id)
        .await?;
    let count = TenantNotificationRepository::new(state.business_db(&claims).await?)
        .unread_count(user_id, false)
        .await?;
    ok(UnreadCountDto { count })
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
    let result = TenantNotificationRepository::new(state.business_db(&claims).await?)
        .list_activity_log(user_id, is_admin, &filters)
        .await?;
    ok(result)
}

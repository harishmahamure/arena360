use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{ok, ApiResult};
use crate::middleware::AdminUser;
use crate::models::{SetStaffGamingAllowanceDto, StaffGamingAllowanceSummary};
use crate::openapi::responses::{ErrorEnvelope, StaffGamingAllowanceSummaryEnvelope};

#[utoipa::path(
    get,
    path = "/users/{id}/staff-gaming-allowance",
    params(
        ("id" = Uuid, Path, description = "Staff user ID"),
    ),
    responses(
        (status = 200, description = "Staff gaming allowance summary", body = StaffGamingAllowanceSummaryEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "staff-gaming-allowance"
)]
pub async fn get_staff_gaming_allowance(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StaffGamingAllowanceSummary> {
    let summary = state.staff_gaming_allowances.get_summary(id).await?;
    ok(summary)
}

#[utoipa::path(
    patch,
    path = "/users/{id}/staff-gaming-allowance",
    params(
        ("id" = Uuid, Path, description = "Staff user ID"),
    ),
    request_body = SetStaffGamingAllowanceDto,
    responses(
        (status = 200, description = "Staff gaming allowance granted or renewed", body = StaffGamingAllowanceSummaryEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "staff-gaming-allowance"
)]
pub async fn update_staff_gaming_allowance(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<SetStaffGamingAllowanceDto>,
) -> ApiResult<StaffGamingAllowanceSummary> {
    let summary = state
        .staff_gaming_allowances
        .grant(id, dto, claims.user_id_uuid())
        .await?;
    ok(summary)
}

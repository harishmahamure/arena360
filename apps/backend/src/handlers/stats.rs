use axum::extract::{Query, State};
use chrono::Duration;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::app::AppState;
use crate::dto::ok;
use crate::middleware::{AdminOrStaff, AdminUser};
use crate::openapi::responses::{
    DashboardStatsEnvelope, ErrorEnvelope, RevenueByPaymentMethodEnvelope,
    StaffDashboardStatsEnvelope, UsageStatsEnvelope,
};
use crate::services::stats_service::{PeriodPair, RevenueByPaymentMethodDto, StatsService, UsageStatsDto};

#[derive(serde::Deserialize, Default, ToSchema, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct StatsQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(serde::Deserialize, Default, ToSchema, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct StaffStatsQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub shift_start: Option<String>,
}

#[utoipa::path(
    get,
    path = "/stats/dashboard",
    params(StatsQuery),
    responses(
        (status = 200, description = "Dashboard statistics", body = DashboardStatsEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "stats"
)]
pub async fn dashboard_stats(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
) -> crate::dto::ApiResult<crate::services::stats_service::DashboardStatsDto> {
    let stats = state
        .stats
        .get_dashboard_stats(query.start_date, query.end_date)
        .await?;
    ok(stats)
}

#[utoipa::path(
    get,
    path = "/stats/staff-dashboard",
    params(StaffStatsQuery),
    responses(
        (status = 200, description = "Staff dashboard statistics", body = StaffDashboardStatsEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "stats"
)]
pub async fn staff_dashboard_stats(
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(query): Query<StaffStatsQuery>,
) -> crate::dto::ApiResult<crate::services::stats_service::StaffDashboardStatsDto> {
    let stats = state
        .stats
        .get_staff_dashboard_stats(query.start_date, query.end_date, query.shift_start)
        .await?;
    ok(stats)
}

#[utoipa::path(
    get,
    path = "/stats/revenue/by-payment-method",
    params(StatsQuery),
    responses(
        (status = 200, description = "Revenue by payment method", body = RevenueByPaymentMethodEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "stats"
)]
pub async fn revenue_by_payment_method(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
) -> crate::dto::ApiResult<PeriodPair<RevenueByPaymentMethodDto>> {
    let (start, end) = StatsService::resolve_stats_period(query.start_date, query.end_date);
    let diff = (end - start).num_days().max(1);
    let prev_start = start - Duration::days(diff);
    let prev_end = end - Duration::days(diff);

    let stats = state
        .stats
        .get_revenue_by_payment_method(start, end, prev_start, prev_end)
        .await?;
    ok(stats)
}

#[utoipa::path(
    get,
    path = "/stats/usage",
    params(StatsQuery),
    responses(
        (status = 200, description = "Usage statistics", body = UsageStatsEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "stats"
)]
pub async fn usage_stats(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
) -> crate::dto::ApiResult<PeriodPair<UsageStatsDto>> {
    let (start, end) = StatsService::resolve_stats_period(query.start_date, query.end_date);
    let diff = (end - start).num_days().max(1);
    let prev_start = start - Duration::days(diff);
    let prev_end = end - Duration::days(diff);

    let stats = state
        .stats
        .get_usage_stats(start, end, prev_start, prev_end)
        .await?;
    ok(stats)
}

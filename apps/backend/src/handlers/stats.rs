use axum::extract::{Query, State};
use chrono::Duration;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::app::AppState;
use crate::dto::ok;
use crate::middleware::{AdminOrStaff, AdminUser};
use crate::openapi::responses::{
    DashboardStatsEnvelope, ErrorEnvelope, FinanceDepositStatsEnvelope,
    FinanceReconciliationStatsEnvelope, FinanceVarianceStatsEnvelope,
    RevenueByPaymentMethodEnvelope, StaffDashboardStatsEnvelope, UsageStatsEnvelope,
};
use crate::services::stats_service::{
    FinanceDepositStatsDto, FinanceReconciliationStatsDto, FinanceVarianceStatsDto, PeriodPair,
    RevenueByPaymentMethodDto, StatsService, UsageStatsDto,
};

#[derive(serde::Deserialize, Default, ToSchema, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct StatsQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    /// When false, previous-period metrics are omitted. Defaults to true.
    pub compare: Option<bool>,
}

#[derive(serde::Deserialize, Default, ToSchema, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct StaffStatsQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub shift_start: Option<String>,
}

fn compare_enabled(compare: Option<bool>) -> bool {
    compare.unwrap_or(true)
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
        .get_dashboard_stats(query.start_date, query.end_date, compare_enabled(query.compare))
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
    let compare = compare_enabled(query.compare);
    let (start, end) = StatsService::resolve_stats_period(query.start_date, query.end_date);
    let diff = (end - start).num_days().max(1);
    let prev_start = start - Duration::days(diff);
    let prev_end = end - Duration::days(diff);

    let stats = state
        .stats
        .get_revenue_by_payment_method(start, end, prev_start, prev_end, compare)
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
    let compare = compare_enabled(query.compare);
    let (start, end) = StatsService::resolve_stats_period(query.start_date, query.end_date);
    let diff = (end - start).num_days().max(1);
    let prev_start = start - Duration::days(diff);
    let prev_end = end - Duration::days(diff);

    let stats = state
        .stats
        .get_usage_stats(start, end, prev_start, prev_end, compare)
        .await?;
    ok(stats)
}

#[utoipa::path(
    get,
    path = "/stats/finance/reconciliation",
    params(StatsQuery),
    responses(
        (status = 200, description = "Finance reconciliation statistics", body = FinanceReconciliationStatsEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "stats"
)]
pub async fn finance_reconciliation_stats(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
) -> crate::dto::ApiResult<FinanceReconciliationStatsDto> {
    ok(state
        .stats
        .get_finance_reconciliation_stats(
            query.start_date,
            query.end_date,
            compare_enabled(query.compare),
        )
        .await?)
}

#[utoipa::path(
    get,
    path = "/stats/finance/deposits",
    params(StatsQuery),
    responses(
        (status = 200, description = "Finance deposit statistics", body = FinanceDepositStatsEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "stats"
)]
pub async fn finance_deposit_stats(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
) -> crate::dto::ApiResult<FinanceDepositStatsDto> {
    ok(state
        .stats
        .get_finance_deposit_stats(
            query.start_date,
            query.end_date,
            compare_enabled(query.compare),
        )
        .await?)
}

#[utoipa::path(
    get,
    path = "/stats/finance/variance",
    params(StatsQuery),
    responses(
        (status = 200, description = "Finance variance statistics", body = FinanceVarianceStatsEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "stats"
)]
pub async fn finance_variance_stats(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
) -> crate::dto::ApiResult<FinanceVarianceStatsDto> {
    ok(state
        .stats
        .get_finance_variance_stats(
            query.start_date,
            query.end_date,
            compare_enabled(query.compare),
        )
        .await?)
}

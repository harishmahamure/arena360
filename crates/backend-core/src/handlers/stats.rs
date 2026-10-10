use axum::extract::{Query, State};
use axum::http::HeaderMap;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::app::AppState;
use crate::middleware::{AdminOrStaff, AdminUser};
use crate::openapi::responses::{
    DashboardStatsEnvelope, ErrorEnvelope, FinanceDepositStatsEnvelope,
    FinanceReconciliationStatsEnvelope, FinanceVarianceStatsEnvelope,
    RevenueByPaymentMethodEnvelope, StaffDashboardStatsEnvelope, UsageStatsEnvelope,
};
use crate::services::stats_service::{
    FinanceDepositStatsDto, FinanceReconciliationStatsDto, FinanceVarianceStatsDto, PeriodPair,
    RevenueByPaymentMethodDto, UsageStatsDto,
};

#[derive(serde::Deserialize, Default, ToSchema, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct StatsQuery {
    pub venue_location_id: Option<Uuid>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    /// When false, previous-period metrics are omitted. Defaults to true.
    pub compare: Option<bool>,
}

#[derive(serde::Deserialize, Default, ToSchema, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct StaffStatsQuery {
    pub venue_location_id: Option<Uuid>,
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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
    headers: HeaderMap,
) -> crate::dto::ApiResult<crate::services::stats_service::DashboardStatsDto> {
    let scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "finance:read",
    )
    .await?;
    let reader = state.report_reader(state.business_db(&claims).await?,scope).await?;
    let stats = crate::services::StatsService::new(reader,state.cache.clone());
    crate::dto::ok(stats.get_dashboard_stats(query.start_date,query.end_date,query.compare.unwrap_or(true)).await?)
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
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(query): Query<StaffStatsQuery>,
    headers: HeaderMap,
) -> crate::dto::ApiResult<crate::services::stats_service::StaffDashboardStatsDto> {
    let scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "stats:read",
    )
    .await?;
    let reader = state.report_reader(state.business_db(&claims).await?,scope).await?;
    let stats = crate::services::StatsService::new(reader,state.cache.clone());
    crate::dto::ok(stats.get_staff_dashboard_stats(query.start_date,query.end_date,query.shift_start).await?)
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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
    headers: HeaderMap,
) -> crate::dto::ApiResult<PeriodPair<RevenueByPaymentMethodDto>> {
    let scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "finance:read",
    )
    .await?;
    let reader = state.report_reader(state.business_db(&claims).await?,scope).await?;
    let stats = crate::services::StatsService::new(reader,state.cache.clone());
    let (start,end) = stats.resolve_stats_period(query.start_date,query.end_date);
    let (previous_start,previous_end) = stats.previous_window(start,end);
    crate::dto::ok(stats.get_revenue_by_payment_method(start,end,previous_start,previous_end,query.compare.unwrap_or(true)).await?)
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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
    headers: HeaderMap,
) -> crate::dto::ApiResult<PeriodPair<UsageStatsDto>> {
    let scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "stats:read",
    )
    .await?;
    let reader = state.report_reader(state.business_db(&claims).await?,scope).await?;
    let stats = crate::services::StatsService::new(reader,state.cache.clone());
    let (start,end) = stats.resolve_stats_period(query.start_date,query.end_date);
    let (previous_start,previous_end) = stats.previous_window(start,end);
    crate::dto::ok(stats.get_usage_stats(start,end,previous_start,previous_end,query.compare.unwrap_or(true)).await?)
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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
    headers: HeaderMap,
) -> crate::dto::ApiResult<FinanceReconciliationStatsDto> {
    let scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "finance:read",
    )
    .await?;
    let reader = state.report_reader(state.business_db(&claims).await?,scope).await?;
    let stats = crate::services::StatsService::new(reader,state.cache.clone());
    crate::dto::ok(stats.get_finance_reconciliation_stats(query.start_date,query.end_date,query.compare.unwrap_or(true)).await?)
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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
    headers: HeaderMap,
) -> crate::dto::ApiResult<FinanceDepositStatsDto> {
    let scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "finance:read",
    )
    .await?;
    let reader = state.report_reader(state.business_db(&claims).await?,scope).await?;
    let stats = crate::services::StatsService::new(reader,state.cache.clone());
    crate::dto::ok(stats.get_finance_deposit_stats(query.start_date,query.end_date,query.compare.unwrap_or(true)).await?)
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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
    headers: HeaderMap,
) -> crate::dto::ApiResult<FinanceVarianceStatsDto> {
    let scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "finance:read",
    )
    .await?;
    let reader = state.report_reader(state.business_db(&claims).await?,scope).await?;
    let stats = crate::services::StatsService::new(reader,state.cache.clone());
    crate::dto::ok(stats.get_finance_variance_stats(query.start_date,query.end_date,query.compare.unwrap_or(true)).await?)
}

#[derive(serde::Deserialize, Default, utoipa::IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessQuery {
    pub venue_location_id: Option<Uuid>,
    /// Inclusive tenant calendar date, YYYY-MM-DD. Defaults to the last 30 days.
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[utoipa::path(get, path = "/stats/business", params(BusinessQuery),
    responses((status = 200, description = "Business analytics from tenant DuckDB", body = crate::openapi::responses::BusinessAnalyticsEnvelope),
    (status = 400, body = ErrorEnvelope), (status = 403, body = ErrorEnvelope), (status = 503, body = ErrorEnvelope)),
    security(("bearer_auth" = [])), tag = "stats")]
pub async fn business_stats(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<BusinessQuery>,
    headers: HeaderMap,
) -> crate::dto::ApiResult<crate::analytics::business::BusinessReport> {
    let scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "finance:read",
    )
    .await?;
    let reader = state.report_reader(state.business_db(&claims).await?,scope).await?;
    let window = crate::analytics::business::Window::new(query.start_date.as_deref(),query.end_date.as_deref(),chrono::Utc::now(),reader.timezone())?;
    let stats = crate::services::StatsService::new(reader,state.cache.clone());
    crate::dto::ok(stats.get_business_report(window).await?)
}

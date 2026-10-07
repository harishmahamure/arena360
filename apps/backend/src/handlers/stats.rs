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
    let _scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "finance:read",
    )
    .await?;
    Err(crate::error::AppError::Api {
        code: "ANALYTICS_UNAVAILABLE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: None,
    })
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
    let _scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "stats:read",
    )
    .await?;
    Err(crate::error::AppError::Api {
        code: "ANALYTICS_UNAVAILABLE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: None,
    })
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
    let _scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "finance:read",
    )
    .await?;
    Err(crate::error::AppError::Api {
        code: "ANALYTICS_UNAVAILABLE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: None,
    })
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
    let _scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "stats:read",
    )
    .await?;
    Err(crate::error::AppError::Api {
        code: "ANALYTICS_UNAVAILABLE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: None,
    })
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
    let _scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "finance:read",
    )
    .await?;
    Err(crate::error::AppError::Api {
        code: "ANALYTICS_UNAVAILABLE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: None,
    })
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
    let _scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "finance:read",
    )
    .await?;
    Err(crate::error::AppError::Api {
        code: "ANALYTICS_UNAVAILABLE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: None,
    })
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
    let _scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "finance:read",
    )
    .await?;
    Err(crate::error::AppError::Api {
        code: "ANALYTICS_UNAVAILABLE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: None,
    })
}

#[derive(serde::Deserialize, Default, utoipa::IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessQuery {
    pub venue_location_id: Option<Uuid>,
    /// Inclusive IST calendar date, YYYY-MM-DD. Defaults to the last 30 days.
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[utoipa::path(get, path = "/stats/business", params(BusinessQuery),
    responses((status = 200, description = "Business analytics from ClickHouse", body = crate::openapi::responses::BusinessAnalyticsEnvelope),
    (status = 400, body = ErrorEnvelope), (status = 403, body = ErrorEnvelope), (status = 503, body = ErrorEnvelope)),
    security(("bearer_auth" = [])), tag = "stats")]
pub async fn business_stats(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<BusinessQuery>,
    headers: HeaderMap,
) -> crate::dto::ApiResult<crate::analytics::business::BusinessReport> {
    let _scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "finance:read",
    )
    .await?;
    Err(crate::error::AppError::Api {
        code: "ANALYTICS_UNAVAILABLE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: None,
    })
}

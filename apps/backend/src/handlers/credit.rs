use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::access::scope::{requested_location, LocationScope};
use crate::app::AppState;
use crate::dto::{created, ok, ApiResult, PaginationResult};
use crate::error::AppError;
use crate::middleware::{require_staff_for_counter, AdminOrStaff, AdminUser};
use crate::models::{
    CreditAccountFilterDto, CreditPlayerRow, CreditPortfolioSummary, CreditSettlement,
    CreditSettlementDetail, CreditSettlementFilterDto, CreditSummary, PlayerCreditDetail,
    SetCreditLimitDto, SettleCreditDto,
};
use crate::openapi::responses::{
    CreditPlayerPaginationEnvelope, CreditPortfolioSummaryEnvelope, CreditSettlementDetailEnvelope,
    CreditSettlementEnvelope, CreditSettlementPaginationEnvelope, CreditSummaryEnvelope,
    ErrorEnvelope, PlayerCreditDetailEnvelope,
};
use crate::repositories::{
    TenantCreditRepository, TenantSettingsRepository, TenantShiftRepository,
};

#[utoipa::path(
    get,
    path = "/credit/accounts",
    params(CreditAccountFilterDto),
    responses(
        (status = 200, description = "List players with outstanding credit", body = CreditPlayerPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "credit"
)]
pub async fn list_credit_accounts(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<CreditAccountFilterDto>,
) -> ApiResult<PaginationResult<CreditPlayerRow>> {
    let db = state.business_db(&claims).await?;
    let actor = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid identity".into()))?;
    TenantSettingsRepository::new(db.clone())
        .ensure_access(db.tenant_id(), actor, "credit:read")
        .await?;
    let result = state
        .credit
        .list_credit_players_tenant(state.business_db(&claims).await?, filters)
        .await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/credit/summary",
    responses(
        (status = 200, description = "Portfolio credit summary (limits vs outstanding)", body = CreditPortfolioSummaryEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "credit"
)]
pub async fn credit_summary(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<CreditPortfolioSummary> {
    let db = state.business_db(&claims).await?;
    let actor = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid identity".into()))?;
    TenantSettingsRepository::new(db.clone())
        .ensure_access(db.tenant_id(), actor, "credit:read")
        .await?;
    let locations = crate::access::scope::report_scope_tenant(db.clone(),&claims,&headers,None,"credit:read").await?;
    let reader=state.report_reader(db,locations).await?;
    ok(state.credit.portfolio_summary(&reader).await?)
}

#[utoipa::path(
    get,
    path = "/credit/players/{id}",
    params(
        ("id" = Uuid, Path, description = "Player ID"),
    ),
    responses(
        (status = 200, description = "Player credit summary and outstanding transactions", body = PlayerCreditDetailEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "credit"
)]
pub async fn get_player_credit(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<PlayerCreditDetail> {
    let db = state.business_db(&claims).await?;
    let actor = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid identity".into()))?;
    TenantSettingsRepository::new(db.clone())
        .ensure_access(db.tenant_id(), actor, "credit:read")
        .await?;
    let detail = state
        .credit
        .get_player_credit_tenant(state.business_db(&claims).await?, id)
        .await?;
    ok(detail)
}

#[utoipa::path(
    get,
    path = "/credit/settlements",
    params(CreditSettlementFilterDto),
    responses(
        (status = 200, description = "List credit settlements", body = CreditSettlementPaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "credit"
)]
pub async fn list_settlements(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(filters): Query<CreditSettlementFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<crate::models::CreditSettlementListRow>> {
    let db = state.business_db(&claims).await?;
    let scope = LocationScope::resolve_tenant(
        db.clone(),
        &claims,
        "credit:read",
        requested_location(&headers)?,
    )
    .await?;
    ok(TenantCreditRepository::new(db)
        .list_settlements_scoped(&filters, Some(&scope.locations))
        .await?)
}

#[utoipa::path(
    get,
    path = "/credit/settlements/{id}",
    params(
        ("id" = Uuid, Path, description = "Settlement ID"),
    ),
    responses(
        (status = 200, description = "Credit settlement detail", body = CreditSettlementDetailEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "credit"
)]
pub async fn get_settlement(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<CreditSettlementDetail> {
    let db = state.business_db(&claims).await?;
    let repo = TenantCreditRepository::new(db.clone());
    let venue = repo.settlement_location_id(id).await?;
    LocationScope::resolve_tenant(db, &claims, "credit:read", Some(venue)).await?;
    ok(repo.settlement_detail(id).await?)
}

#[utoipa::path(
    post,
    path = "/credit/settlements",
    request_body = SettleCreditDto,
    responses(
        (status = 201, description = "Credit settlement recorded", body = CreditSettlementEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "credit"
)]
pub async fn create_settlement(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<SettleCreditDto>,
) -> ApiResult<CreditSettlement> {
    let db = state.business_db(&claims).await?;
    let user_id = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::BadRequest("Invalid user ID in token".to_string()))?;

    require_staff_for_counter(&claims)?;

    let active_shift =
        crate::repositories::TenantShiftRepository::new(state.business_db(&claims).await?)
            .find_active_by_user(user_id)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest("No active shift found for current user".to_string())
            })?;

    let venue = TenantShiftRepository::new(db.clone())
        .location_id(active_shift.id)
        .await?;
    LocationScope::resolve_tenant(db.clone(), &claims, "credit:write", Some(venue)).await?;
    let settlement = state
        .credit
        .settle_tenant(db, dto, active_shift.id, user_id)
        .await?;
    created(settlement)
}

#[utoipa::path(
    patch,
    path = "/users/{id}/credit-limit",
    params(
        ("id" = Uuid, Path, description = "Player ID"),
    ),
    request_body = SetCreditLimitDto,
    responses(
        (status = 200, description = "Credit limit updated", body = CreditSummaryEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "credit"
)]
pub async fn update_credit_limit(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<SetCreditLimitDto>,
) -> ApiResult<CreditSummary> {
    let db = state.business_db(&claims).await?;
    let actor = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid identity".into()))?;
    TenantSettingsRepository::new(db.clone())
        .ensure_access(db.tenant_id(), actor, "credit-limit:write")
        .await?;
    let summary = state
        .credit
        .set_limit_tenant(
            state.business_db(&claims).await?,
            id,
            dto,
            claims.user_id_uuid(),
        )
        .await?;
    ok(summary)
}

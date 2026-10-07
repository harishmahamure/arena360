use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::access::scope::{requested_location, require_tenant_admin, LocationScope};
use crate::app::AppState;
use crate::dto::{created, ok, ApiResult, PaginationResult};
use crate::middleware::{AdminOrStaff, AdminUser};
use crate::models::{
    ApproveExpenseDto, CreateExpenseDto, Expense, ExpenseFilterDto, ExpenseSummaryDto,
    RejectExpenseDto, UpdateExpenseDto,
};
use crate::openapi::responses::{
    ErrorEnvelope, ExpenseEnvelope, ExpensePaginationEnvelope, ExpenseSummaryListEnvelope,
};
use crate::repositories::TenantExpenseRepository;
use crate::{dto::JwtUserClaims, error::AppError, tenancy::TenantDb};

#[utoipa::path(
    get,
    path = "/expenses",
    responses(
        (status = 200, description = "List expenses", body = ExpensePaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "expenses"
)]
pub async fn list_expenses(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(filters): Query<ExpenseFilterDto>,
) -> ApiResult<PaginationResult<Expense>> {
    let db = state.business_db(&claims).await?;
    let selected = requested_location(&headers)?;
    let scope =
        LocationScope::resolve_tenant(db.clone(), &claims, "expenses:read", selected).await?;
    let result = TenantExpenseRepository::new(db)
        .list_scoped(
            &filters,
            &scope.locations,
            scope.organization_admin && selected.is_none(),
        )
        .await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/expenses/{id}",
    params(
        ("id" = Uuid, Path, description = "Expense ID"),
    ),
    responses(
        (status = 200, description = "Get expense", body = ExpenseEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "expenses"
)]
pub async fn get_expense(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Expense> {
    let db = state.business_db(&claims).await?;
    let expected_venue = authorize_expense(db.clone(), &claims, id, "expenses:read").await?;
    let expense = TenantExpenseRepository::new(db.clone())
        .find_by_id_if_location(id, expected_venue)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("Expense not found".into()))?;
    ok(expense)
}

#[utoipa::path(
    post,
    path = "/expenses",
    request_body = CreateExpenseDto,
    responses(
        (status = 201, description = "Create expense", body = ExpenseEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "expenses"
)]
pub async fn create_expense(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(dto): Json<CreateExpenseDto>,
) -> ApiResult<Expense> {
    let db = state.business_db(&claims).await?;
    let selected = requested_location(&headers)?;
    let shift_venue = match dto.shift_id {
        Some(id) => Some(
            crate::repositories::TenantShiftRepository::new(db.clone())
                .location_id(id)
                .await?,
        ),
        None => None,
    };
    if selected.is_some() && shift_venue.is_some() && selected != shift_venue {
        return Err(AppError::BadRequest(
            "Expense venue must match its shift".into(),
        ));
    }
    let scope = LocationScope::resolve_tenant(
        db.clone(),
        &claims,
        "expenses:write",
        selected.or(shift_venue),
    )
    .await?;
    let venue = if selected.or(shift_venue).is_some() || scope.locations.len() == 1 {
        Some(scope.first()?)
    } else if scope.organization_admin {
        None
    } else {
        return Err(AppError::bad_request_code("LOCATION_REQUIRED", None));
    };
    let user_id = Uuid::parse_str(&claims.userId).ok();
    let expense = TenantExpenseRepository::new(db.clone())
        .create_at(&dto, user_id, venue)
        .await?;
    created(expense)
}

#[utoipa::path(
    patch,
    path = "/expenses/{id}",
    params(
        ("id" = Uuid, Path, description = "Expense ID"),
    ),
    request_body = UpdateExpenseDto,
    responses(
        (status = 200, description = "Update expense", body = ExpenseEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "expenses"
)]
pub async fn update_expense(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateExpenseDto>,
) -> ApiResult<Expense> {
    let db = state.business_db(&claims).await?;
    let expected_venue = authorize_expense(db.clone(), &claims, id, "expenses:write").await?;
    if let Some(shift) = dto.shift_id {
        let venue = crate::repositories::TenantShiftRepository::new(db.clone())
            .location_id(shift)
            .await?;
        LocationScope::resolve_tenant(db.clone(), &claims, "expenses:write", Some(venue)).await?;
    }
    let user_id = Uuid::parse_str(&claims.userId).ok();
    let expense = TenantExpenseRepository::new(db.clone())
        .update_if_location(id, &dto, user_id, expected_venue)
        .await?;
    ok(expense)
}

#[utoipa::path(
    patch,
    path = "/expenses/{id}/approve",
    params(
        ("id" = Uuid, Path, description = "Expense ID"),
    ),
    request_body = ApproveExpenseDto,
    responses(
        (status = 200, description = "Approve expense", body = ExpenseEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "expenses"
)]
pub async fn approve_expense(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(_dto): Json<ApproveExpenseDto>,
) -> ApiResult<Expense> {
    let db = state.business_db(&claims).await?;
    let expected_venue = authorize_expense(db.clone(), &claims, id, "expenses:approve").await?;
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    let expense = TenantExpenseRepository::new(db.clone())
        .approve_if_location(id, user_id, expected_venue)
        .await?;
    ok(expense)
}

#[utoipa::path(
    patch,
    path = "/expenses/{id}/reject",
    params(
        ("id" = Uuid, Path, description = "Expense ID"),
    ),
    request_body = RejectExpenseDto,
    responses(
        (status = 200, description = "Reject expense", body = ExpenseEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "expenses"
)]
pub async fn reject_expense(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<RejectExpenseDto>,
) -> ApiResult<Expense> {
    let db = state.business_db(&claims).await?;
    let expected_venue = authorize_expense(db.clone(), &claims, id, "expenses:approve").await?;
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    let expense = TenantExpenseRepository::new(db.clone())
        .reject_if_location(id, &dto.rejection_reason, user_id, expected_venue)
        .await?;
    ok(expense)
}

#[utoipa::path(
    get,
    path = "/expenses/summary",
    responses(
        (status = 200, description = "Get expense summary by category", body = ExpenseSummaryListEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "expenses"
)]
pub async fn expense_summary(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Vec<ExpenseSummaryDto>> {
    let db = state.business_db(&claims).await?;
    LocationScope::resolve_tenant(
        db.clone(),
        &claims,
        "expenses:read",
        requested_location(&headers)?,
    )
    .await?;
    let summary = TenantExpenseRepository::new(db.clone())
        .get_summary()
        .await?;
    ok(summary)
}

#[utoipa::path(
    delete,
    path = "/expenses/{id}",
    params(
        ("id" = Uuid, Path, description = "Expense ID"),
    ),
    responses(
        (status = 200, description = "Delete expense", body = ExpenseEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "expenses"
)]
pub async fn delete_expense(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Expense> {
    let db = state.business_db(&claims).await?;
    let expected_venue = authorize_expense(db.clone(), &claims, id, "expenses:write").await?;
    let expense = TenantExpenseRepository::new(db.clone())
        .soft_delete_if_location(id, expected_venue)
        .await?;
    ok(expense)
}

async fn authorize_expense(
    db: Arc<TenantDb>,
    claims: &JwtUserClaims,
    id: Uuid,
    permission: &str,
) -> Result<Option<Uuid>, AppError> {
    let venue = TenantExpenseRepository::new(db.clone())
        .location_id(id)
        .await?;
    let scope = LocationScope::resolve_tenant(db.clone(), claims, permission, venue).await?;
    if venue.is_none() {
        require_tenant_admin(db, scope.organization_id, scope.user_id).await?;
    }
    Ok(venue)
}

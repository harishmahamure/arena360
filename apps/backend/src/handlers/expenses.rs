use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

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
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<ExpenseFilterDto>,
) -> ApiResult<PaginationResult<Expense>> {
    let result = state.expenses.list(filters).await?;
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
    AdminOrStaff(_claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Expense> {
    let expense = state.expenses.get_by_id(id).await?;
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
    Json(dto): Json<CreateExpenseDto>,
) -> ApiResult<Expense> {
    let user_id = Uuid::parse_str(&claims.userId).ok();
    let expense = state.expenses.create(dto, user_id).await?;
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
    let user_id = Uuid::parse_str(&claims.userId).ok();
    let expense = state.expenses.update(id, dto, user_id).await?;
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
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    let expense = state.expenses.approve(id, user_id).await?;
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
    let user_id = Uuid::parse_str(&claims.userId)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    let expense = state
        .expenses
        .reject(id, &dto.rejection_reason, user_id)
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
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Vec<ExpenseSummaryDto>> {
    let summary = state.expenses.get_summary().await?;
    ok(summary)
}

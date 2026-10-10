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
    CreateExpenseCategoryDto, ExpenseCategory, ExpenseCategoryFilterDto, UpdateExpenseCategoryDto,
};
use crate::openapi::responses::{
    ErrorEnvelope, ExpenseCategoryEnvelope, ExpenseCategoryPaginationEnvelope,
};
use crate::repositories::TenantExpenseCategoryRepository;

#[utoipa::path(
    get,
    path = "/expense-categories",
    responses(
        (status = 200, description = "List expense categories", body = ExpenseCategoryPaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "expense-categories"
)]
pub async fn list_expense_categories(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(filters): Query<ExpenseCategoryFilterDto>,
) -> ApiResult<PaginationResult<ExpenseCategory>> {
    let result = TenantExpenseCategoryRepository::new(state.business_db(&claims).await?)
        .list(&filters)
        .await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/expense-categories/{id}",
    params(
        ("id" = Uuid, Path, description = "Expense category ID"),
    ),
    responses(
        (status = 200, description = "Get expense category", body = ExpenseCategoryEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "expense-categories"
)]
pub async fn get_expense_category(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<ExpenseCategory> {
    let category = TenantExpenseCategoryRepository::new(state.business_db(&claims).await?)
        .find_by_id(id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("Expense category not found".into()))?;
    ok(category)
}

#[utoipa::path(
    post,
    path = "/expense-categories",
    request_body = CreateExpenseCategoryDto,
    responses(
        (status = 201, description = "Create expense category", body = ExpenseCategoryEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 409, description = "Conflict", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "expense-categories"
)]
pub async fn create_expense_category(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateExpenseCategoryDto>,
) -> ApiResult<ExpenseCategory> {
    let user_id = Uuid::parse_str(&claims.userId).ok();
    let category = TenantExpenseCategoryRepository::new(state.business_db(&claims).await?)
        .create(&dto, user_id)
        .await?;
    created(category)
}

#[utoipa::path(
    patch,
    path = "/expense-categories/{id}",
    params(
        ("id" = Uuid, Path, description = "Expense category ID"),
    ),
    request_body = UpdateExpenseCategoryDto,
    responses(
        (status = 200, description = "Update expense category", body = ExpenseCategoryEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 409, description = "Conflict", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "expense-categories"
)]
pub async fn update_expense_category(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateExpenseCategoryDto>,
) -> ApiResult<ExpenseCategory> {
    let user_id = Uuid::parse_str(&claims.userId).ok();
    let category = TenantExpenseCategoryRepository::new(state.business_db(&claims).await?)
        .update(id, &dto, user_id)
        .await?;
    ok(category)
}

#[utoipa::path(
    delete,
    path = "/expense-categories/{id}",
    params(
        ("id" = Uuid, Path, description = "Expense category ID"),
    ),
    responses(
        (status = 200, description = "Delete expense category", body = serde_json::Value),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "expense-categories"
)]
pub async fn delete_expense_category(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<serde_json::Value> {
    TenantExpenseCategoryRepository::new(state.business_db(&claims).await?)
        .delete(id)
        .await?;
    ok(serde_json::json!({"deleted": true}))
}

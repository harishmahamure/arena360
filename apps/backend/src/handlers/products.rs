use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::middleware::AdminUser;
use crate::models::{CreateProductDto, Product, ProductFilterDto, UpdateProductDto};
use crate::openapi::responses::{ErrorEnvelope, ProductEnvelope, ProductPaginationEnvelope};

#[utoipa::path(
    get,
    path = "/products",
    responses(
        (status = 200, description = "List products", body = ProductPaginationEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "products"
)]
pub async fn list_products(
    State(state): State<Arc<AppState>>,
    Query(filters): Query<ProductFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<Product>> {
    let result = state.products.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/products/{id}",
    params(
        ("id" = Uuid, Path, description = "Product ID"),
    ),
    responses(
        (status = 200, description = "Get product", body = ProductEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "products"
)]
pub async fn get_product(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Product> {
    let product = state.products.get_by_id(id).await?;
    ok(product)
}

#[utoipa::path(
    post,
    path = "/products",
    request_body = CreateProductDto,
    responses(
        (status = 201, description = "Create product", body = ProductEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "products"
)]
pub async fn create_product(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateProductDto>,
) -> ApiResult<Product> {
    let product = state.products.create(dto).await?;
    created(product)
}

#[utoipa::path(
    patch,
    path = "/products/{id}",
    params(
        ("id" = Uuid, Path, description = "Product ID"),
    ),
    request_body = UpdateProductDto,
    responses(
        (status = 200, description = "Update product", body = ProductEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "products"
)]
pub async fn update_product(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateProductDto>,
) -> ApiResult<Product> {
    let product = state.products.update(id, dto).await?;
    ok(product)
}

#[utoipa::path(
    delete,
    path = "/products/{id}",
    params(
        ("id" = Uuid, Path, description = "Product ID"),
    ),
    responses(
        (status = 200, description = "Soft-deactivated (isActive=false)", body = ProductEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "products"
)]
pub async fn delete_product(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Product> {
    let product = state.products.delete(id).await?;
    ok(product)
}

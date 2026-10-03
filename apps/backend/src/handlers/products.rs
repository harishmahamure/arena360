use crate::access::scope::{requested_location, LocationScope};
use crate::services::catalog_scope::{self, CatalogCreate};
use axum::http::HeaderMap;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::error::AppError;
use crate::middleware::{AdminUser, AuthUser};
use crate::models::{
    CreateProductDto, CurrentPricesQuery, Product, ProductCurrentPrice, ProductFilterDto,
    ProductRecipe, UpdateProductDto,
};
use crate::openapi::responses::{
    ErrorEnvelope, ProductCurrentPriceListEnvelope, ProductEnvelope, ProductPaginationEnvelope,
    ProductRecipeEnvelope,
};

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
    AuthUser(claims): AuthUser,
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Query(mut filters): Query<ProductFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<Product>> {
    let scope = LocationScope::resolve(
        &state.db,
        &claims,
        "products:read",
        requested_location(&headers)?,
    )
    .await?;
    filters.organization_id = Some(scope.organization_id);
    filters.allowed_location_ids = Some(scope.locations);
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
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Product> {
    let scope = LocationScope::resolve(&state.db, &claims, "products:read", None).await?;
    catalog_scope::get(&state.db, "products", id, &scope, false).await?;

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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<CatalogCreate<CreateProductDto>>,
) -> ApiResult<Product> {
    let scope = LocationScope::resolve(&state.db, &claims, "products:write", None).await?;
    let requested = payload.location_ids.or(if scope.organization_admin {
        None
    } else {
        requested_location(&headers)?.map(|id| vec![id])
    });
    let locations = catalog_scope::create_locations(&state.db, &scope, requested).await?;
    let dto = payload.item;
    let service = crate::services::ProductService::new(state.db.clone(), state.cache.clone())
        .with_locations(locations);

    let product = service.create(dto, claims.user_id_uuid()).await?;
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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateProductDto>,
) -> ApiResult<Product> {
    let scope = LocationScope::resolve(&state.db, &claims, "products:write", None).await?;
    catalog_scope::get(&state.db, "products", id, &scope, true).await?;

    let product = state
        .products
        .update(id, dto, claims.user_id_uuid())
        .await?;
    ok(product)
}

#[utoipa::path(
    get,
    path = "/products/current-prices",
    params(CurrentPricesQuery),
    responses(
        (status = 200, description = "Sale price of each product right now, after published product pricing rules and before options", body = ProductCurrentPriceListEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "products"
)]
pub async fn current_prices(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<CurrentPricesQuery>,
    headers: HeaderMap,
) -> ApiResult<Vec<ProductCurrentPrice>> {
    let venue_location_id = if claims.is_admin_or_staff() {
        let org = Uuid::parse_str(&claims.tenantId)
            .map_err(|_| AppError::Forbidden("Select an organization".into()))?;
        let user = claims
            .user_id_uuid()
            .ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))?;
        let location = LocationScope::resolve(
            &state.db,
            &claims,
            "products:read",
            query.venue_location_id.or(requested_location(&headers)?),
        )
        .await?
        .first()?;
        state
            .config
            .ensure_location_permission(org, location, user, "products:read")
            .await?;
        location
    } else if let Some(device_id) = claims.deviceId.as_deref() {
        state
            .devices
            .get_by_id(
                Uuid::parse_str(device_id)
                    .map_err(|_| AppError::Unauthorized("Invalid device identity".into()))?,
            )
            .await?
            .location_id
    } else {
        crate::models::DEFAULT_VENUE_LOCATION_ID
    };
    if let Some(store_id) = query.location_id {
        let store = state.inventory.get_location(store_id).await?;
        if store.venue_location_id != venue_location_id {
            return Err(AppError::BadRequest(
                "Store location must belong to the selected venue".into(),
            ));
        }
    }
    ok(state
        .transactions
        .current_product_prices(query.location_id, Some(venue_location_id))
        .await?)
}

#[utoipa::path(
    get,
    path = "/products/{id}/recipe",
    params(("id" = Uuid, Path, description = "Product ID")),
    responses(
        (status = 200, description = "Recipe ingredients and options", body = ProductRecipeEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "products"
)]
pub async fn get_recipe(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<ProductRecipe> {
    let scope = LocationScope::resolve(&state.db, &claims, "products:read", None).await?;
    catalog_scope::get(&state.db, "products", id, &scope, false).await?;

    ok(state.product_recipes.get(id).await?)
}

#[utoipa::path(
    put,
    path = "/products/{id}/recipe",
    params(("id" = Uuid, Path, description = "Product ID")),
    request_body = ProductRecipe,
    responses(
        (status = 200, description = "Recipe replaced", body = ProductRecipeEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "products"
)]
pub async fn save_recipe(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(recipe): Json<ProductRecipe>,
) -> ApiResult<ProductRecipe> {
    let scope = LocationScope::resolve(&state.db, &claims, "products:write", None).await?;
    catalog_scope::get(&state.db, "products", id, &scope, true).await?;

    ok(state.product_recipes.save(id, recipe).await?)
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
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Product> {
    let scope = LocationScope::resolve(&state.db, &claims, "products:write", None).await?;
    catalog_scope::get(&state.db, "products", id, &scope, true).await?;

    let product = state.products.delete(id).await?;
    ok(product)
}

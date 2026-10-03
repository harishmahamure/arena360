use crate::{
    access::scope::LocationScope,
    app::AppState,
    dto::{ok, ApiResult, JwtUserClaims},
    error::AppError,
    middleware::AdminOrStaff,
    services::catalog_scope::{self, CatalogScope},
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogScopeView {
    #[serde(flatten)]
    scope: CatalogScope,
    writable_location_ids: Vec<Uuid>,
}

async fn view(
    state: &AppState,
    claims: &JwtUserClaims,
    kind: &str,
    id: Uuid,
) -> Result<CatalogScopeView, AppError> {
    let read = LocationScope::resolve(&state.db, claims, &format!("{kind}:read"), None).await?;
    let scope = catalog_scope::get(&state.db, kind, id, &read, false).await?;
    let writable_location_ids =
        match LocationScope::resolve(&state.db, claims, &format!("{kind}:write"), None).await {
            Ok(write) => write.locations,
            Err(AppError::Forbidden(_)) => vec![],
            Err(error) => return Err(error),
        };
    Ok(CatalogScopeView {
        scope,
        writable_location_ids,
    })
}
use uuid::Uuid;

pub async fn get_product(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<CatalogScopeView> {
    ok(view(&state, &claims, "products", id).await?)
}
pub async fn save_product(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<CatalogScope>,
) -> ApiResult<CatalogScope> {
    let scope = LocationScope::resolve(&state.db, &claims, "products:write", None).await?;
    let result = catalog_scope::save(&state.db, "products", id, &scope, dto).await?;
    state.cache.invalidate_prefix("products:").await?;
    ok(result)
}
pub async fn get_plan(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<CatalogScopeView> {
    ok(view(&state, &claims, "plans", id).await?)
}
pub async fn save_plan(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<CatalogScope>,
) -> ApiResult<CatalogScope> {
    let scope = LocationScope::resolve(&state.db, &claims, "plans:write", None).await?;
    let result = catalog_scope::save(&state.db, "plans", id, &scope, dto).await?;
    state.cache.invalidate_prefix("plans:").await?;
    ok(result)
}

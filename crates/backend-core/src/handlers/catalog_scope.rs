use crate::{
    access::scope::LocationScope,
    app::AppState,
    dto::{ok, ApiResult, JwtUserClaims},
    error::AppError,
    middleware::AdminOrStaff,
    repositories::{TenantPlanRepository, TenantProductRepository},
    services::catalog_scope::{self, CatalogScope, LocationPrice},
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
    let read = LocationScope::resolve_tenant(
        state.business_db(claims).await?,
        claims,
        &format!("{kind}:read"),
        None,
    )
    .await?;
    let scope = {
        let db = state.business_db(claims).await?;
        let (all, rows) = match kind {
            "products" => TenantProductRepository::new(db).location_scope(id).await?,
            "plans" => TenantPlanRepository::new(db).location_scope(id).await?,
            _ => return Err(AppError::NotFound("Unknown catalog".into())),
        };
        let location_ids = if all {
            vec![]
        } else {
            rows.iter().map(|(location_id, _)| *location_id).collect()
        };
        catalog_scope::authorize(&read, &location_ids, false)?;
        CatalogScope {
            location_ids,
            prices: rows
                .into_iter()
                .filter(|(location_id, price)| {
                    price.is_some()
                        && (read.organization_admin || read.locations.contains(location_id))
                })
                .map(|(location_id, price)| LocationPrice {
                    location_id,
                    price: price.expect("filtered to priced rows"),
                })
                .collect(),
        }
    };
    let writable_location_ids = match LocationScope::resolve_tenant(
        state.business_db(claims).await?,
        claims,
        &format!("{kind}:write"),
        None,
    )
    .await
    {
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
    let scope = LocationScope::resolve_tenant(
        state.business_db(&claims).await?,
        &claims,
        "products:write",
        None,
    )
    .await?;
    let db = state.business_db(&claims).await?;
    catalog_scope::save_tenant_scope(&scope, "products", id, dto, db).await?;
    ok(view(&state, &claims, "products", id).await?.scope)
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
    let scope = LocationScope::resolve_tenant(
        state.business_db(&claims).await?,
        &claims,
        "plans:write",
        None,
    )
    .await?;
    let db = state.business_db(&claims).await?;
    catalog_scope::save_tenant_scope(&scope, "plans", id, dto, db).await?;
    ok(view(&state, &claims, "plans", id).await?.scope)
}

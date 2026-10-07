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
    let read = LocationScope::resolve(&state.db, claims, &format!("{kind}:read"), None).await?;
    let scope = if let Some(db) = state.tenant_db(read.organization_id).await? {
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
    } else {
        catalog_scope::get(&state.db, kind, id, &read, false).await?
    };
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
    if let Some(db) = state.tenant_db(scope.organization_id).await? {
        save_tenant_scope(&state, &scope, "products", id, dto, db).await?;
        state.cache.invalidate_prefix("products:").await?;
        return ok(view(&state, &claims, "products", id).await?.scope);
    }
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
    if let Some(db) = state.tenant_db(scope.organization_id).await? {
        save_tenant_scope(&state, &scope, "plans", id, dto, db).await?;
        state.cache.invalidate_prefix("plans:").await?;
        return ok(view(&state, &claims, "plans", id).await?.scope);
    }
    let result = catalog_scope::save(&state.db, "plans", id, &scope, dto).await?;
    state.cache.invalidate_prefix("plans:").await?;
    ok(result)
}

async fn save_tenant_scope(
    state: &AppState,
    scope: &LocationScope,
    kind: &str,
    id: Uuid,
    dto: CatalogScope,
    db: Arc<crate::tenancy::TenantDb>,
) -> Result<(), AppError> {
    if dto.location_ids.len() > 200 || dto.prices.len() > 200 {
        return Err(AppError::BadRequest("Too many locations".into()));
    }
    let (all, old_rows) = match kind {
        "products" => {
            TenantProductRepository::new(db.clone())
                .location_scope(id)
                .await?
        }
        "plans" => {
            TenantPlanRepository::new(db.clone())
                .location_scope(id)
                .await?
        }
        _ => return Err(AppError::NotFound("Unknown catalog".into())),
    };
    let old_locations = if all {
        vec![]
    } else {
        old_rows
            .iter()
            .map(|(location_id, _)| *location_id)
            .collect()
    };
    let old_location_set = old_locations
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let new_location_set = dto
        .location_ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let availability_unchanged =
        all == dto.location_ids.is_empty() && old_location_set == new_location_set;
    if availability_unchanged {
        catalog_scope::authorize(scope, &old_locations, false)?;
    } else {
        catalog_scope::authorize(scope, &old_locations, true)?;
        catalog_scope::authorize(scope, &dto.location_ids, true)?;
    }
    if !scope.organization_admin
        && dto
            .prices
            .iter()
            .any(|price| !scope.locations.contains(&price.location_id))
    {
        return Err(AppError::Forbidden(
            "You can only change prices at your assigned locations".into(),
        ));
    }
    let active: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM venue_locations
           WHERE "organizationId"=$1 AND "isActive""#,
    )
    .bind(scope.organization_id)
    .fetch_all(&state.db)
    .await?;
    if dto.location_ids.iter().any(|id| !active.contains(id))
        || dto.prices.iter().any(|price| {
            !price.price.is_finite()
                || price.price < 0.0
                || !active.contains(&price.location_id)
                || (!dto.location_ids.is_empty() && !dto.location_ids.contains(&price.location_id))
        })
    {
        return Err(AppError::BadRequest("Choose active locations in this business and nonnegative prices within the item's availability".into()));
    }
    let mut unique = std::collections::HashSet::new();
    if dto
        .prices
        .iter()
        .any(|price| !unique.insert(price.location_id))
    {
        return Err(AppError::BadRequest(
            "Each location can have only one price".into(),
        ));
    }
    let prices = dto
        .prices
        .into_iter()
        .map(|price| (price.location_id, price.price))
        .collect();
    match kind {
        "products" => {
            let repo = TenantProductRepository::new(db);
            if !scope.organization_admin && availability_unchanged {
                repo.replace_authorized_location_prices(
                    id,
                    dto.location_ids,
                    prices,
                    scope.locations.clone(),
                )
                .await
            } else {
                repo.replace_location_scope(id, dto.location_ids, prices)
                    .await
            }
        }
        "plans" => {
            let repo = TenantPlanRepository::new(db);
            if !scope.organization_admin && availability_unchanged {
                repo.replace_authorized_location_prices(
                    id,
                    dto.location_ids,
                    prices,
                    scope.locations.clone(),
                )
                .await
            } else {
                repo.replace_location_scope(id, dto.location_ids, prices)
                    .await
            }
        }
        _ => unreachable!("kind checked above"),
    }
}

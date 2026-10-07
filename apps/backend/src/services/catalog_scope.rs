use crate::{access::scope::LocationScope, error::AppError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocationPrice {
    pub location_id: Uuid,
    pub price: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogScope {
    pub location_ids: Vec<Uuid>,
    pub prices: Vec<LocationPrice>,
}
use crate::repositories::{TenantPlanRepository, TenantProductRepository};
use crate::tenancy::TenantDb;
use std::sync::Arc;

async fn rows(
    db: Arc<TenantDb>,
    kind: &str,
    id: Uuid,
) -> Result<(bool, Vec<(Uuid, Option<f64>)>), AppError> {
    match kind {
        "products" => TenantProductRepository::new(db).location_scope(id).await,
        "plans" => TenantPlanRepository::new(db).location_scope(id).await,
        _ => Err(AppError::NotFound("Unknown catalog".into())),
    }
}
pub async fn available(
    db: Arc<TenantDb>,
    kind: &str,
    id: Uuid,
    location: Uuid,
) -> Result<Option<f64>, AppError> {
    crate::repositories::TenantSettingsRepository::new(db.clone())
        .validate_location(db.tenant_id(), location)
        .await?;
    let (all, rows) = rows(db, kind, id).await?;
    if let Some((_, price)) = rows.iter().find(|(venue, _)| *venue == location) {
        return Ok(*price);
    }
    if all {
        Ok(None)
    } else {
        Err(AppError::BadRequest(
            "This catalog item is not available at the selected location".into(),
        ))
    }
}
pub async fn get(
    db: Arc<TenantDb>,
    kind: &str,
    id: Uuid,
    scope: &LocationScope,
    write: bool,
) -> Result<CatalogScope, AppError> {
    let (all, rows) = rows(db, kind, id).await?;
    let location_ids = if all {
        vec![]
    } else {
        rows.iter().map(|row| row.0).collect()
    };
    authorize(scope, &location_ids, write)?;
    let prices = rows
        .into_iter()
        .filter(|(venue, price)| {
            price.is_some() && (scope.organization_admin || scope.locations.contains(venue))
        })
        .map(|(location_id, price)| LocationPrice {
            location_id,
            price: price.unwrap(),
        })
        .collect();
    Ok(CatalogScope {
        location_ids,
        prices,
    })
}
pub async fn save(
    db: Arc<TenantDb>,
    kind: &str,
    id: Uuid,
    scope: &LocationScope,
    dto: CatalogScope,
) -> Result<(), AppError> {
    save_tenant_scope(scope, kind, id, dto, db).await
}

pub fn authorize(scope: &LocationScope, locations: &[Uuid], write: bool) -> Result<(), AppError> {
    if scope.organization_admin
        || (!write
            && (locations.is_empty() || locations.iter().any(|id| scope.locations.contains(id))))
        || (write
            && !locations.is_empty()
            && locations.iter().all(|id| scope.locations.contains(id)))
    {
        Ok(())
    } else {
        Err(AppError::Forbidden("This catalog item is shared beyond your assigned locations; ask an organization administrator to edit it".into()))
    }
}
pub async fn save_tenant_scope(
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
        authorize(scope, &old_locations, false)?;
    } else {
        authorize(scope, &old_locations, true)?;
        authorize(scope, &dto.location_ids, true)?;
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
        "SELECT unhex(replace(id,'-','')) FROM venue_locations WHERE is_active=1",
    )
    .fetch_all(&db.read_pool()?)
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCreate<T> {
    #[serde(flatten)]
    pub item: T,
    pub location_ids: Option<Vec<Uuid>>,
}
pub async fn create_locations_tenant(
    db: std::sync::Arc<crate::tenancy::TenantDb>,
    scope: &LocationScope,
    requested: Option<Vec<Uuid>>,
) -> Result<Vec<Uuid>, AppError> {
    let mut ids = requested.unwrap_or_else(|| {
        if scope.organization_admin {
            vec![]
        } else {
            scope.locations.clone()
        }
    });
    ids.sort();
    ids.dedup();
    authorize(scope, &ids, true)?;
    let settings = crate::repositories::TenantSettingsRepository::new(db);
    for id in &ids {
        settings
            .validate_location(scope.organization_id, *id)
            .await?;
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staff_can_read_shared_items_but_cannot_edit_outside_their_locations() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let scope = LocationScope {
            organization_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            organization_admin: false,
            locations: vec![a],
        };
        assert!(authorize(&scope, &[], false).is_ok());
        assert!(authorize(&scope, &[], true).is_err());
        assert!(authorize(&scope, &[a], true).is_ok());
        assert!(authorize(&scope, &[b], false).is_err());
        assert!(authorize(&scope, &[a, b], true).is_err());
    }
}

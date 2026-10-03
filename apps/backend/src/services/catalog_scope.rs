use crate::{access::scope::LocationScope, error::AppError};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
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
pub fn table(kind: &str) -> Result<(&'static str, &'static str, &'static str), AppError> {
    match kind {
        "products" => Ok(("products", "product_location_prices", "productId")),
        "plans" => Ok(("plans", "plan_location_prices", "planId")),
        _ => Err(AppError::NotFound("Unknown catalog".into())),
    }
}
pub async fn get(
    pool: &PgPool,
    kind: &str,
    id: Uuid,
    scope: &LocationScope,
    write: bool,
) -> Result<CatalogScope, AppError> {
    let (table, prices, key) = table(kind)?;
    let locations: Vec<Uuid> = sqlx::query_scalar(&format!(r#"SELECT "locationIds" FROM {table} WHERE id=$1 AND "organizationId"=$2 AND "deletedAt" IS NULL"#))
        .bind(id).bind(scope.organization_id).fetch_optional(pool).await?.ok_or_else(|| AppError::NotFound("Catalog item not found".into()))?;
    authorize(scope, &locations, write)?;
    let rows: Vec<(Uuid, f64)> = sqlx::query_as(&format!(r#"SELECT "locationId",price::float8 FROM {prices} WHERE "{key}"=$1 AND "organizationId"=$2 AND ($3 OR "locationId"=ANY($4)) ORDER BY "locationId""#))
        .bind(id).bind(scope.organization_id).bind(scope.organization_admin).bind(&scope.locations).fetch_all(pool).await?;
    Ok(CatalogScope {
        location_ids: locations,
        prices: rows
            .into_iter()
            .map(|(location_id, price)| LocationPrice { location_id, price })
            .collect(),
    })
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
pub async fn save(
    pool: &PgPool,
    kind: &str,
    id: Uuid,
    scope: &LocationScope,
    dto: CatalogScope,
) -> Result<CatalogScope, AppError> {
    let (table, prices, key) = table(kind)?;
    if dto.location_ids.len() > 200 || dto.prices.len() > 200 {
        return Err(AppError::BadRequest("Too many locations".into()));
    }
    let mut tx = pool.begin().await?;
    let old: Vec<Uuid> = sqlx::query_scalar(&format!(r#"SELECT "locationIds" FROM {table} WHERE id=$1 AND "organizationId"=$2 AND "deletedAt" IS NULL FOR UPDATE"#))
        .bind(id).bind(scope.organization_id).fetch_optional(&mut *tx).await?.ok_or_else(|| AppError::NotFound("Catalog item not found".into()))?;
    if old == dto.location_ids {
        authorize(scope, &old, false)?;
    } else {
        authorize(scope, &old, true)?;
        authorize(scope, &dto.location_ids, true)?;
    }
    if !scope.organization_admin
        && dto
            .prices
            .iter()
            .any(|p| !scope.locations.contains(&p.location_id))
    {
        return Err(AppError::Forbidden(
            "You can only change prices at your assigned locations".into(),
        ));
    }
    let active: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM venue_locations WHERE "organizationId"=$1 AND "isActive""#,
    )
    .bind(scope.organization_id)
    .fetch_all(&mut *tx)
    .await?;
    if dto.location_ids.iter().any(|id| !active.contains(id))
        || dto.prices.iter().any(|p| {
            !p.price.is_finite()
                || p.price < 0.0
                || !active.contains(&p.location_id)
                || (!dto.location_ids.is_empty() && !dto.location_ids.contains(&p.location_id))
        })
    {
        return Err(AppError::BadRequest("Choose active locations in this business and nonnegative prices within the item's availability".into()));
    }
    let mut unique = std::collections::HashSet::new();
    if dto.prices.iter().any(|p| !unique.insert(p.location_id)) {
        return Err(AppError::BadRequest(
            "Each location can have only one price".into(),
        ));
    }
    sqlx::query(&format!(r#"UPDATE {table} SET "locationIds"=$3,"updatedAt"=now() WHERE id=$1 AND "organizationId"=$2"#)).bind(id).bind(scope.organization_id).bind(&dto.location_ids).execute(&mut *tx).await?;
    sqlx::query(&format!(r#"DELETE FROM {prices} WHERE "{key}"=$1 AND "organizationId"=$2 AND ($3 OR "locationId"=ANY($4))"#)).bind(id).bind(scope.organization_id).bind(scope.organization_admin).bind(&scope.locations).execute(&mut *tx).await?;
    for price in &dto.prices {
        sqlx::query(&format!(r#"INSERT INTO {prices} ("organizationId","{key}","locationId",price) VALUES($1,$2,$3,$4)"#))
            .bind(scope.organization_id).bind(id).bind(price.location_id).bind(price.price).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(dto)
}
pub async fn available(
    pool: &PgPool,
    kind: &str,
    id: Uuid,
    location: Uuid,
) -> Result<Option<f64>, AppError> {
    let (table, prices, key) = table(kind)?;
    let row: Option<(Option<f64>,)> = sqlx::query_as(&format!(r#"SELECT p.price::float8 FROM {table} c JOIN venue_locations l ON l."organizationId"=c."organizationId" AND l.id=$2
        LEFT JOIN {prices} p ON p."{key}"=c.id AND p."locationId"=l.id
        WHERE c.id=$1 AND c."deletedAt" IS NULL AND (cardinality(c."locationIds")=0 OR l.id=ANY(c."locationIds"))"#))
        .bind(id).bind(location).fetch_optional(pool).await?;
    row.map(|r| r.0).ok_or_else(|| {
        AppError::BadRequest("This catalog item is not available at the selected location".into())
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCreate<T> {
    #[serde(flatten)]
    pub item: T,
    pub location_ids: Option<Vec<Uuid>>,
}
pub async fn create_locations(
    pool: &PgPool,
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
    let count: i64=sqlx::query_scalar(r#"SELECT count(*) FROM venue_locations WHERE "organizationId"=$1 AND "isActive" AND id=ANY($2)"#)
        .bind(scope.organization_id).bind(&ids).fetch_one(pool).await?;
    if count != ids.len() as i64 {
        return Err(AppError::BadRequest(
            "Choose active locations in this business".into(),
        ));
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

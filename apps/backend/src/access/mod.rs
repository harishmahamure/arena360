use crate::{dto::JwtUserClaims, error::AppError};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;
pub mod routes;
pub mod scope;
pub const MANAGED: &str = "system:managed-access-v1";
pub fn catalog() -> Value {
    serde_json::from_str(include_str!("catalog.json")).expect("checked-in access catalog")
}
pub fn has(claims: &JwtUserClaims, permission: &str) -> bool {
    claims.permissions.iter().any(|p| p == permission)
}
pub fn managed(claims: &JwtUserClaims) -> bool {
    has(claims, MANAGED)
}
pub fn known(permission: &str) -> bool {
    static GRANTS: std::sync::OnceLock<std::collections::HashSet<String>> =
        std::sync::OnceLock::new();
    GRANTS
        .get_or_init(|| {
            catalog()
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|m| {
                    m["permissions"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| p["key"].as_str().unwrap().to_string())
                })
                .collect()
        })
        .contains(permission)
}
pub async fn effective<'e>(
    pool: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    org: Uuid,
    user: Uuid,
) -> Result<Vec<String>, sqlx::Error> {
    let mut values:Vec<String>=sqlx::query_scalar(r#"SELECT DISTINCT p.permission FROM access_assignments a
        JOIN organizations o ON o.id=a.organization_id AND o."isActive"
        JOIN access_roles r ON r.id=a.role_id AND r.organization_id=a.organization_id AND NOT r.is_template
        JOIN organization_memberships om ON om."organizationId"=a.organization_id AND om."userId"=a.user_id AND om."isActive"
        CROSS JOIN LATERAL jsonb_array_elements_text(r.permissions) p(permission)
        LEFT JOIN access_modules m ON m.organization_id=a.organization_id AND m.module=split_part(p.permission,':',1)
        WHERE a.organization_id=$1 AND a.user_id=$2 AND coalesce(m.enabled,true) ORDER BY p.permission"#)
        .bind(org).bind(user).fetch_all(pool).await?;
    values.retain(|p| known(p));
    values.push(MANAGED.into());
    values.sort();
    Ok(values)
}
pub async fn require(
    pool: &PgPool,
    org: Uuid,
    user: Uuid,
    permission: &str,
) -> Result<(), AppError> {
    if effective(pool, org, user)
        .await?
        .iter()
        .any(|p| p == permission)
    {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!(
            "Permission required: {permission}"
        )))
    }
}

pub async fn require_location(
    pool: &PgPool,
    org: Uuid,
    user: Uuid,
    location: Uuid,
    permission: &str,
) -> Result<(), AppError> {
    let admin: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(SELECT 1 FROM organization_memberships
        WHERE "organizationId"=$1 AND "userId"=$2 AND "isActive" AND role='admin')"#,
    )
    .bind(org)
    .bind(user)
    .fetch_one(pool)
    .await?;
    if admin {
        require(pool, org, user, permission).await?;
        let active: bool = sqlx::query_scalar(r#"SELECT EXISTS(SELECT 1 FROM venue_locations WHERE id=$1 AND "organizationId"=$2 AND "isActive")"#)
            .bind(location).bind(org).fetch_one(pool).await?;
        return if active {
            Ok(())
        } else {
            Err(AppError::Forbidden(
                "Choose an active location in this organization".into(),
            ))
        };
    }
    let allowed: bool = sqlx::query_scalar(r#"SELECT EXISTS(
        SELECT 1 FROM location_role_assignments a
        JOIN organization_memberships m ON m."organizationId"=a.organization_id AND m."userId"=a.user_id AND m."isActive"
        JOIN venue_locations l ON l.id=a.location_id AND l."organizationId"=a.organization_id AND l."isActive"
        JOIN location_access_assignments la ON la."organizationId"=a.organization_id AND la."locationId"=a.location_id AND la."membershipId"=m.id
        JOIN access_roles r ON r.id=a.role_id AND r.organization_id=a.organization_id AND NOT r.is_template
        LEFT JOIN access_modules mod ON mod.organization_id=a.organization_id AND mod.module=split_part($4,':',1)
        WHERE a.organization_id=$1 AND a.user_id=$2 AND a.location_id=$3
          AND r.permissions ? $4 AND coalesce(mod.enabled,true)
    )"#).bind(org).bind(user).bind(location).bind(permission).fetch_one(pool).await?;
    if allowed {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!(
            "Permission required at this location: {permission}"
        )))
    }
}

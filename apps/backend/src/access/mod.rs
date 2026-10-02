use crate::{dto::JwtUserClaims,error::AppError};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;
pub mod routes;
pub const MANAGED: &str = "system:managed-access-v1";
pub fn catalog() -> Value { serde_json::from_str(include_str!("catalog.json")).expect("checked-in access catalog") }
pub fn has(claims:&JwtUserClaims, permission:&str)->bool { claims.permissions.iter().any(|p|p==permission) }
pub fn managed(claims:&JwtUserClaims)->bool { has(claims,MANAGED) }
pub fn known(permission:&str)->bool { catalog().as_array().unwrap().iter().any(|m|m["permissions"].as_array().unwrap().iter().any(|p|p["key"]==permission)) }
pub async fn effective(pool:&PgPool,org:Uuid,user:Uuid)->Result<Vec<String>,sqlx::Error> {
    let mut values:Vec<String>=sqlx::query_scalar(r#"SELECT DISTINCT p.permission FROM access_assignments a
        JOIN access_roles r ON r.id=a.role_id AND r.organization_id=a.organization_id AND NOT r.is_template
        JOIN organization_memberships om ON om."organizationId"=a.organization_id AND om."userId"=a.user_id AND om."isActive"
        CROSS JOIN LATERAL jsonb_array_elements_text(r.permissions) p(permission)
        LEFT JOIN access_modules m ON m.organization_id=a.organization_id AND m.module=split_part(p.permission,':',1)
        WHERE a.organization_id=$1 AND a.user_id=$2 AND coalesce(m.enabled,true) ORDER BY p.permission"#)
        .bind(org).bind(user).fetch_all(pool).await?;
    values.retain(|p|known(p));
    values.push(MANAGED.into());values.sort();Ok(values)
}
pub async fn require(pool:&PgPool,org:Uuid,user:Uuid,permission:&str)->Result<(),AppError>{
    if effective(pool,org,user).await?.iter().any(|p|p==permission){Ok(())}else{Err(AppError::Forbidden(format!("Permission required: {permission}")))}
}

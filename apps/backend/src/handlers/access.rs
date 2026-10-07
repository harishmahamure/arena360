use crate::{
    access,
    app::AppState,
    dto::{ok, ApiResult, JwtUserClaims},
    error::AppError,
    middleware::AdminOrStaff,
    repositories::{TenantAccessRepository, TenantRoleDto, TenantSettingsRepository},
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;
fn org(c: &JwtUserClaims) -> Result<Uuid, AppError> {
    Uuid::parse_str(&c.tenantId).map_err(|_| AppError::Forbidden("Select an organization".into()))
}
async fn local_access(
    s: &AppState,
    c: &JwtUserClaims,
    permission: Option<&str>,
) -> Result<Arc<crate::tenancy::TenantDb>, AppError> {
    let db = s.business_db(c).await?;
    let user = c
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user".into()))?;
    let settings = TenantSettingsRepository::new(db.clone());
    if settings.membership_context(user).await?.is_none() {
        return Err(AppError::Forbidden(
            "Active tenant membership required".into(),
        ));
    }
    if let Some(permission) = permission {
        settings.ensure_access(org(c)?, user, permission).await?;
    }
    Ok(db)
}
pub async fn self_access(
    AdminOrStaff(c): AdminOrStaff,
    State(s): State<Arc<AppState>>,
) -> ApiResult<Value> {
    let db = local_access(&s, &c, None).await?;
    let user = c
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user".into()))?;
    let roles: Vec<String> = sqlx::query_scalar("SELECT r.name FROM access_roles r JOIN access_assignments a ON a.role_id=r.id WHERE a.user_id=? AND r.is_template=0 ORDER BY r.name")
        .bind(user.to_string()).fetch_all(&db.read_pool()?).await?;
    let permissions = TenantSettingsRepository::new(db.clone())
        .effective_permissions(user)
        .await?;
    let member = TenantSettingsRepository::new(db)
        .membership_context(user)
        .await?
        .ok_or_else(|| AppError::Forbidden("Active membership required".into()))?;
    ok(
        json!({"roles":roles,"permissions":permissions,"organizationId":org(&c)?,"organizationAdmin":member.role == "admin"}),
    )
}
pub async fn snapshot(
    AdminOrStaff(c): AdminOrStaff,
    State(s): State<Arc<AppState>>,
) -> ApiResult<Value> {
    let db = local_access(&s, &c, Some("access:read")).await?;
    ok(TenantAccessRepository::new(db).snapshot().await?)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoleDto {
    name: String,
    description: String,
    permissions: Vec<String>,
    is_template: bool,
    expected_revision: Option<i32>,
}
fn validate_role(dto: &RoleDto) -> Result<(), AppError> {
    if dto.name.trim().is_empty()
        || dto.name.trim().chars().count() > 80
        || dto.description.len() > 1000
        || dto.permissions.len() > 100
        || dto.permissions.iter().any(|p| !access::known(p))
    {
        return Err(AppError::BadRequest("Provide a role name (1–80 characters), description (up to 1000 characters), and valid permissions".into()));
    }
    Ok(())
}
async fn write_role(
    s: Arc<AppState>,
    c: JwtUserClaims,
    id: Option<Uuid>,
    dto: RoleDto,
) -> ApiResult<Value> {
    validate_role(&dto)?;
    let db = local_access(&s, &c, Some("access:manage")).await?;
    let actor = c
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user".into()))?;
    let id = TenantAccessRepository::new(db)
        .save_role(
            id,
            TenantRoleDto {
                name: dto.name,
                description: dto.description,
                permissions: dto.permissions,
                is_template: dto.is_template,
                expected_revision: dto.expected_revision,
            },
            actor,
        )
        .await?;
    ok(json!({"id":id}))
}

pub async fn create_role(
    AdminOrStaff(c): AdminOrStaff,
    State(s): State<Arc<AppState>>,
    Json(dto): Json<RoleDto>,
) -> ApiResult<Value> {
    if dto.expected_revision.is_some() {
        return Err(AppError::BadRequest(
            "A new role cannot have a revision".into(),
        ));
    }
    write_role(s, c, None, dto).await
}
pub async fn update_role(
    AdminOrStaff(c): AdminOrStaff,
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<RoleDto>,
) -> ApiResult<Value> {
    if dto.expected_revision.is_none() {
        return Err(AppError::BadRequest("Expected revision is required".into()));
    }
    write_role(s, c, Some(id), dto).await
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevisionDto {
    expected_revision: i32,
}
pub async fn delete_role(
    AdminOrStaff(c): AdminOrStaff,
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<RevisionDto>,
) -> ApiResult<Value> {
    let db = local_access(&s, &c, Some("access:manage")).await?;
    let actor = c
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user".into()))?;
    TenantAccessRepository::new(db)
        .delete_role(id, dto.expected_revision, actor)
        .await?;
    ok(json!({"deleted":true}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemberDto {
    role_ids: Vec<Uuid>,
    location_ids: Option<Vec<Uuid>>,
    location_roles: Option<Vec<LocationRoleDto>>,
    active: bool,
    expected_revision: i32,
}
#[derive(Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocationRoleDto {
    location_id: Uuid,
    role_ids: Vec<Uuid>,
}
pub async fn save_member(
    AdminOrStaff(c): AdminOrStaff,
    State(s): State<Arc<AppState>>,
    Path(user): Path<Uuid>,
    Json(dto): Json<MemberDto>,
) -> ApiResult<Value> {
    let db = local_access(&s, &c, Some("access:manage")).await?;
    let actor = c
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user".into()))?;
    TenantAccessRepository::new(db)
        .save_member_assignments(
            user,
            crate::repositories::TenantMemberDto {
                role_ids: dto.role_ids,
                location_ids: dto.location_ids,
                location_roles: dto.location_roles.map(|rows| {
                    rows.into_iter()
                        .map(|row| crate::repositories::TenantLocationRoleDto {
                            location_id: row.location_id,
                            role_ids: row.role_ids,
                        })
                        .collect()
                }),
                active: dto.active,
                expected_revision: i64::from(dto.expected_revision),
            },
            actor,
        )
        .await?;
    ok(json!({"saved":true}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModuleDto {
    enabled: bool,
    expected_revision: i32,
}
pub async fn save_module(
    AdminOrStaff(c): AdminOrStaff,
    State(s): State<Arc<AppState>>,
    Path(module): Path<String>,
    Json(dto): Json<ModuleDto>,
) -> ApiResult<Value> {
    let db = local_access(&s, &c, Some("access:manage")).await?;
    let actor = c
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user".into()))?;
    TenantAccessRepository::new(db)
        .save_module(&module, dto.enabled, dto.expected_revision, actor)
        .await?;
    ok(json!({"saved":true}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewMember {
    username: String,
    password: String,
    role_ids: Vec<Uuid>,
    #[serde(default)]
    location_ids: Vec<Uuid>,
    location_roles: Option<Vec<LocationRoleDto>>,
}
pub async fn create_member(
    AdminOrStaff(c): AdminOrStaff,
    State(s): State<Arc<AppState>>,
    Json(dto): Json<NewMember>,
) -> ApiResult<Value> {
    let db = local_access(&s, &c, Some("access:manage")).await?;
    let actor = c
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user".into()))?;
    let username = dto.username.trim().to_lowercase();
    if !(3..=80).contains(&username.len())
        || !username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        || !(12..=72).contains(&dto.password.len())
    {
        return Err(AppError::BadRequest(
            "Use a 3–80 character username and a 12–72 byte password".into(),
        ));
    }
    if dto.location_ids.is_empty() || dto.location_ids.len() > 100 || dto.role_ids.len() > 20 {
        return Err(AppError::BadRequest(
            "Choose roles and between one and 100 active locations".into(),
        ));
    }
    let ids: std::collections::HashSet<_> = dto.location_ids.iter().collect();
    let roles: std::collections::HashSet<_> = dto.role_ids.iter().collect();
    if ids.len() != dto.location_ids.len() || roles.len() != dto.role_ids.len() {
        return Err(AppError::BadRequest(
            "Choose unique roles and locations".into(),
        ));
    }
    for id in &dto.location_ids {
        TenantSettingsRepository::new(db.clone())
            .validate_location(db.tenant_id(), *id)
            .await?;
    }
    for role in &dto.role_ids {
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM access_roles WHERE id=? AND is_template=0)",
        )
        .bind(role.to_string())
        .fetch_one(&db.read_pool()?)
        .await?;
        if !valid {
            return Err(AppError::BadRequest(
                "Choose tenant roles; templates cannot be assigned".into(),
            ));
        }
    }
    if let Some(scopes) = &dto.location_roles {
        let unique: std::collections::HashSet<_> = scopes.iter().map(|v| v.location_id).collect();
        if scopes.len() != dto.location_ids.len()
            || unique.len() != scopes.len()
            || scopes.iter().any(|v| {
                !dto.location_ids.contains(&v.location_id)
                    || v.role_ids.iter().any(|r| !dto.role_ids.contains(r))
                    || v.role_ids
                        .iter()
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        != v.role_ids.len()
            })
        {
            return Err(AppError::BadRequest(
                "Provide matching roles for every selected location".into(),
            ));
        }
    }
    let pool = s.control_db.clone().ok_or_else(|| AppError::Api {
        code: "CONTROL_AUTH_UNAVAILABLE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: None,
    })?;
    if crate::repositories::TenantUserRepository::new(db.clone())
        .username_exists(&username, None)
        .await?
    {
        // A retry can find its disabled projection. The control-plane pending record
        // below still proves the creation belongs to this actor and tenant.
        let local: Option<String> = sqlx::query_scalar(
            "SELECT role FROM users WHERE lower(username)=lower(?) AND deleted_at IS NULL",
        )
        .bind(&username)
        .fetch_optional(&db.read_pool()?)
        .await?;
        if local.as_deref() == Some("player") {
            return Err(AppError::Conflict(
                "Username already in use in this tenant".into(),
            ));
        }
    }
    let id = crate::control::identity::IdentityRepository::new(pool.clone())
        .create_disabled_staff(db.tenant_id(), actor, &username, &dto.password)
        .await?;
    crate::control::staff_projection::sync_tenant(&pool, db.clone()).await?;
    let revision: i64 = sqlx::query_scalar("SELECT access_revision FROM users WHERE id=?")
        .bind(id.to_string())
        .fetch_one(&db.read_pool()?)
        .await?;
    if revision == 0 {
        TenantAccessRepository::new(db.clone())
            .save_member_assignments(
                id,
                crate::repositories::TenantMemberDto {
                    role_ids: dto.role_ids,
                    location_ids: Some(dto.location_ids),
                    location_roles: dto.location_roles.map(|rows| {
                        rows.into_iter()
                            .map(|r| crate::repositories::TenantLocationRoleDto {
                                location_id: r.location_id,
                                role_ids: r.role_ids,
                            })
                            .collect()
                    }),
                    active: true,
                    expected_revision: 0,
                },
                actor,
            )
            .await?;
    }
    crate::control::staff_projection::sync_tenant(&pool, db).await?;
    ok(json!({"id":id}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn role_permissions_cannot_forge_internal_authority() {
        let mut dto = RoleDto {
            name: "Kitchen".into(),
            description: "".into(),
            permissions: vec!["kitchen:read".into()],
            is_template: false,
            expected_revision: None,
        };
        assert!(validate_role(&dto).is_ok());
        dto.permissions.push(access::MANAGED.into());
        assert!(validate_role(&dto).is_err());
    }
}

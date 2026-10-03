use crate::{
    access,
    app::AppState,
    dto::{ok, ApiResult, JwtUserClaims},
    error::AppError,
    middleware::AdminOrStaff,
    realtime::OutboxService,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use std::sync::Arc;
use uuid::Uuid;
fn org(c: &JwtUserClaims) -> Result<Uuid, AppError> {
    Uuid::parse_str(&c.tenantId).map_err(|_| AppError::Forbidden("Select an organization".into()))
}
async fn lock(tx: &mut Transaction<'_, Postgres>, org: Uuid) -> Result<(), AppError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,44))")
        .bind(org.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(())
}
async fn keep_administrator(tx: &mut Transaction<'_, Postgres>, org: Uuid) -> Result<(), AppError> {
    let exists:bool=sqlx::query_scalar(r#"SELECT EXISTS(SELECT 1 FROM access_assignments a JOIN access_roles r ON r.id=a.role_id AND NOT r.is_template
 JOIN users u ON u.id=a.user_id AND u."isActive" AND u."deletedAt" IS NULL
 JOIN organization_memberships m ON m."organizationId"=a.organization_id AND m."userId"=a.user_id AND m."isActive"
 WHERE a.organization_id=$1 AND r.permissions ? 'access:manage')"#).bind(org).fetch_one(&mut **tx).await?;
    if !exists {
        return Err(AppError::Conflict(
            "Keep at least one active member with access management permission".into(),
        ));
    }
    Ok(())
}
async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    claims: &JwtUserClaims,
    action: &str,
    target: &str,
    before: Value,
    after: Value,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO access_audit(organization_id,actor_id,action,target_id,before_value,after_value) VALUES($1,$2,$3,$4,$5,$6)")
 .bind(org).bind(claims.user_id_uuid()).bind(action).bind(target).bind(before).bind(after).execute(&mut **tx).await?;
    let users: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT "userId" FROM organization_memberships WHERE "organizationId"=$1 AND "isActive""#,
    )
    .bind(org)
    .fetch_all(&mut **tx)
    .await?;
    for user in users {
        OutboxService::publish_in_tx(
            tx,
            &format!("user:{user}"),
            "access.changed",
            json!({"organizationId":org}),
            None,
            Some(user),
            None,
            true,
        )
        .await?;
    }
    Ok(())
}
pub async fn self_access(
    AdminOrStaff(c): AdminOrStaff,
    State(s): State<Arc<AppState>>,
) -> ApiResult<Value> {
    let org = org(&c)?;
    let roles:Vec<String>=sqlx::query_scalar("SELECT r.name FROM access_roles r JOIN access_assignments a ON a.role_id=r.id WHERE a.organization_id=$1 AND a.user_id=$2 ORDER BY r.name")
 .bind(org).bind(c.user_id_uuid()).fetch_all(&s.db).await?;
    let organization_admin = crate::access::scope::is_organization_admin(
        &s.db,
        org,
        c.user_id_uuid()
            .ok_or_else(|| AppError::Unauthorized("Invalid user".into()))?,
    )
    .await?;
    ok(
        json!({"roles":roles,"permissions":c.permissions,"organizationId":org,"organizationAdmin":organization_admin}),
    )
}
pub async fn snapshot(
    AdminOrStaff(c): AdminOrStaff,
    State(s): State<Arc<AppState>>,
) -> ApiResult<Value> {
    let org = org(&c)?;
    let roles:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'name',name,'description',description,'permissions',permissions,'isTemplate',is_template,'revision',revision,'memberCount',(SELECT count(*) FROM access_assignments a WHERE a.role_id=r.id)) FROM access_roles r WHERE organization_id=$1 ORDER BY is_template,name").bind(org).fetch_all(&s.db).await?;
    let members:Vec<Value>=sqlx::query_scalar(r#"SELECT jsonb_build_object('id',u.id,'username',u.username,'name',concat_ws(' ',u."firstName",u."lastName"),'active',m."isActive" AND u."isActive",'revision',coalesce(v.revision,0),
 'roleIds',(SELECT coalesce(jsonb_agg(a.role_id),'[]'::jsonb) FROM access_assignments a WHERE a.organization_id=$1 AND a.user_id=u.id),
 'locationIds',(SELECT coalesce(jsonb_agg(la."locationId"),'[]'::jsonb) FROM location_access_assignments la WHERE la."membershipId"=m.id),
 'locationRoles',(SELECT coalesce(jsonb_agg(jsonb_build_object('locationId',lr.location_id,'roleIds',lr.roles)),'[]'::jsonb) FROM (SELECT location_id,jsonb_agg(role_id) roles FROM location_role_assignments WHERE organization_id=$1 AND user_id=u.id GROUP BY location_id) lr))
 FROM organization_memberships m JOIN users u ON u.id=m."userId"
 LEFT JOIN access_member_versions v ON v.organization_id=m."organizationId" AND v.user_id=u.id
 WHERE m."organizationId"=$1 AND u.role IN ('admin','staff') AND u."deletedAt" IS NULL ORDER BY u.username"#).bind(org).fetch_all(&s.db).await?;
    let modules:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('key',module,'enabled',enabled,'revision',revision) FROM access_modules WHERE organization_id=$1").bind(org).fetch_all(&s.db).await?;
    let audit:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',a.id,'action',a.action,'targetId',a.target_id,'actor',u.username,'before',a.before_value,'after',a.after_value,'at',a.created_at) FROM access_audit a LEFT JOIN users u ON u.id=a.actor_id WHERE a.organization_id=$1 ORDER BY a.id DESC LIMIT 100").bind(org).fetch_all(&s.db).await?;
    ok(
        json!({"catalog":access::catalog(),"roles":roles,"members":members,"modules":modules,"audit":audit}),
    )
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
    let org = org(&c)?;
    let mut tx = s.db.begin().await?;
    lock(&mut tx, org).await?;
    let id = id.unwrap_or_else(Uuid::new_v4);
    let before: Option<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(r) FROM access_roles r WHERE organization_id=$1 AND id=$2",
    )
    .bind(org)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    if dto.expected_revision
        != before
            .as_ref()
            .and_then(|r| r["revision"].as_i64())
            .map(|r| r as i32)
    {
        return Err(AppError::Conflict(
            "Role changed or no longer exists. Reload before saving.".into(),
        ));
    }
    if let Some(old) = &before {
        if old["is_template"] != dto.is_template {
            return Err(AppError::BadRequest(
                "Role/template type cannot be changed; create a copy instead".into(),
            ));
        }
    }
    let duplicate:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM access_roles WHERE organization_id=$1 AND lower(name)=lower($2) AND is_template=$3 AND id<>$4)")
 .bind(org).bind(dto.name.trim()).bind(dto.is_template).bind(id).fetch_one(&mut *tx).await?;
    if duplicate {
        return Err(AppError::Conflict(
            "That role or template name already exists".into(),
        ));
    }
    let mut permissions = dto.permissions;
    for grant in permissions.clone() {
        if let Some((module, _)) = grant.split_once(':') {
            let read = format!("{module}:read");
            if access::known(&read) {
                permissions.push(read);
            }
        }
    }
    permissions.sort();
    permissions.dedup();
    sqlx::query("INSERT INTO access_roles(id,organization_id,name,description,permissions,is_template) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(id) DO UPDATE SET name=$3,description=$4,permissions=$5,revision=access_roles.revision+1,updated_at=now()")
 .bind(id).bind(org).bind(dto.name.trim()).bind(dto.description.trim()).bind(json!(permissions)).bind(dto.is_template).execute(&mut *tx).await?;
    keep_administrator(&mut tx, org).await?;
    audit(
        &mut tx,
        org,
        &c,
        "role.saved",
        &id.to_string(),
        before.unwrap_or(Value::Null),
        json!({"name":dto.name.trim(),"permissions":permissions,"isTemplate":dto.is_template}),
    )
    .await?;
    tx.commit().await?;
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
    let org = org(&c)?;
    let mut tx = s.db.begin().await?;
    lock(&mut tx, org).await?;
    let before: Value = sqlx::query_scalar(
        "SELECT to_jsonb(r) FROM access_roles r WHERE organization_id=$1 AND id=$2 AND revision=$3",
    )
    .bind(org)
    .bind(id)
    .bind(dto.expected_revision)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::Conflict("Role changed. Reload before deleting.".into()))?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM access_assignments WHERE role_id=$1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    if count > 0 {
        return Err(AppError::Conflict(
            "Reassign every member before deleting this role".into(),
        ));
    }
    sqlx::query("DELETE FROM access_roles WHERE organization_id=$1 AND id=$2")
        .bind(org)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    keep_administrator(&mut tx, org).await?;
    audit(
        &mut tx,
        org,
        &c,
        "role.deleted",
        &id.to_string(),
        before,
        Value::Null,
    )
    .await?;
    tx.commit().await?;
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
async fn assign_location_roles(
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    user: Uuid,
    locations: &[Uuid],
    global_roles: &[Uuid],
    scopes: &[LocationRoleDto],
) -> Result<(), AppError> {
    if scopes.len() != locations.len() {
        return Err(AppError::BadRequest(
            "Provide roles for every selected location".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for scope in scopes {
        if !locations.contains(&scope.location_id) || !seen.insert(scope.location_id) {
            return Err(AppError::BadRequest(
                "Location role assignments must match selected locations".into(),
            ));
        }
        validate_roles(tx, org, &scope.role_ids).await?;
        if scope
            .role_ids
            .iter()
            .any(|role| !global_roles.contains(role))
        {
            return Err(AppError::BadRequest(
                "Location roles must be included in member roles".into(),
            ));
        }
    }
    sqlx::query("DELETE FROM location_role_assignments WHERE organization_id=$1 AND user_id=$2")
        .bind(org)
        .bind(user)
        .execute(&mut **tx)
        .await?;
    for scope in scopes {
        for role in &scope.role_ids {
            sqlx::query("INSERT INTO location_role_assignments(organization_id,user_id,location_id,role_id) VALUES($1,$2,$3,$4)")
                .bind(org).bind(user).bind(scope.location_id).bind(role).execute(&mut **tx).await?;
        }
    }
    Ok(())
}
async fn validate_locations(
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    ids: &[Uuid],
) -> Result<(), AppError> {
    if ids.is_empty() || ids.len() > 100 {
        return Err(AppError::BadRequest(
            "Choose between one and 100 locations".into(),
        ));
    }
    let count: i64 = sqlx::query_scalar(r#"SELECT count(*) FROM venue_locations WHERE "organizationId"=$1 AND "isActive" AND id=ANY($2)"#)
        .bind(org).bind(ids).fetch_one(&mut **tx).await?;
    if count != ids.len() as i64 {
        return Err(AppError::BadRequest(
            "Choose unique active locations in this organization".into(),
        ));
    }
    Ok(())
}
async fn assign_locations(
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    user: Uuid,
    ids: &[Uuid],
) -> Result<(), AppError> {
    validate_locations(tx, org, ids).await?;
    let membership: Uuid = sqlx::query_scalar(
        r#"SELECT id FROM organization_memberships WHERE "organizationId"=$1 AND "userId"=$2"#,
    )
    .bind(org)
    .bind(user)
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query(r#"DELETE FROM location_access_assignments WHERE "membershipId"=$1"#)
        .bind(membership)
        .execute(&mut **tx)
        .await?;
    for id in ids {
        sqlx::query(r#"INSERT INTO location_access_assignments("organizationId","membershipId","locationId") VALUES($1,$2,$3)"#)
            .bind(org).bind(membership).bind(id).execute(&mut **tx).await?;
    }
    Ok(())
}
async fn validate_roles(
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    ids: &[Uuid],
) -> Result<(), AppError> {
    if ids.len() > 20 {
        return Err(AppError::BadRequest("Choose at most 20 roles".into()));
    }
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM access_roles WHERE organization_id=$1 AND id=ANY($2) AND NOT is_template").bind(org).bind(ids).fetch_one(&mut **tx).await?;
    if count != ids.len() as i64 {
        return Err(AppError::BadRequest(
            "Choose unique roles from this organization; templates cannot be assigned".into(),
        ));
    }
    Ok(())
}
pub async fn save_member(
    AdminOrStaff(c): AdminOrStaff,
    State(s): State<Arc<AppState>>,
    Path(user): Path<Uuid>,
    Json(dto): Json<MemberDto>,
) -> ApiResult<Value> {
    let org = org(&c)?;
    let mut tx = s.db.begin().await?;
    lock(&mut tx, org).await?;
    validate_roles(&mut tx, org, &dto.role_ids).await?;
    if let Some(ref ids) = dto.location_ids {
        validate_locations(&mut tx, org, ids).await?;
    }
    let before:Option<Value>=sqlx::query_scalar(r#"SELECT jsonb_build_object('active',m."isActive",'roleIds',(SELECT coalesce(jsonb_agg(role_id),'[]'::jsonb) FROM access_assignments WHERE organization_id=$1 AND user_id=$2),'locationIds',(SELECT coalesce(jsonb_agg("locationId"),'[]'::jsonb) FROM location_access_assignments WHERE "membershipId"=m.id)) FROM organization_memberships m JOIN users u ON u.id=m."userId" WHERE m."organizationId"=$1 AND m."userId"=$2 AND u.role IN ('admin','staff') AND u."deletedAt" IS NULL"#).bind(org).bind(user).fetch_optional(&mut *tx).await?;
    let before = before
        .ok_or_else(|| AppError::NotFound("Team member not found in this organization".into()))?;
    let revision: Option<i32> = sqlx::query_scalar(
        "SELECT revision FROM access_member_versions WHERE organization_id=$1 AND user_id=$2",
    )
    .bind(org)
    .bind(user)
    .fetch_optional(&mut *tx)
    .await?;
    if revision.unwrap_or(0) != dto.expected_revision {
        return Err(AppError::Conflict(
            "Member changed. Reload before saving.".into(),
        ));
    }
    sqlx::query("DELETE FROM access_assignments WHERE organization_id=$1 AND user_id=$2")
        .bind(org)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    for role in &dto.role_ids {
        sqlx::query(
            "INSERT INTO access_assignments(organization_id,user_id,role_id) VALUES($1,$2,$3)",
        )
        .bind(org)
        .bind(user)
        .bind(role)
        .execute(&mut *tx)
        .await?;
    }
    if let Some(ref ids) = dto.location_ids {
        assign_locations(&mut tx, org, user, ids).await?;
    }
    if let Some(ref scopes) = dto.location_roles {
        let ids = dto.location_ids.as_ref().ok_or_else(|| {
            AppError::BadRequest("Locations are required with location roles".into())
        })?;
        assign_location_roles(&mut tx, org, user, ids, &dto.role_ids, scopes).await?;
    } else if let Some(ref ids) = dto.location_ids {
        let scopes: Vec<LocationRoleDto> = ids
            .iter()
            .map(|id| LocationRoleDto {
                location_id: *id,
                role_ids: dto.role_ids.clone(),
            })
            .collect();
        assign_location_roles(&mut tx, org, user, ids, &dto.role_ids, &scopes).await?;
    } else {
        // Older clients only send organization roles. Keep their location grants in sync.
        let ids: Vec<Uuid> = sqlx::query_scalar(r#"SELECT la."locationId" FROM location_access_assignments la JOIN organization_memberships m ON m.id=la."membershipId" WHERE m."organizationId"=$1 AND m."userId"=$2"#)
            .bind(org).bind(user).fetch_all(&mut *tx).await?;
        let scopes: Vec<LocationRoleDto> = ids
            .iter()
            .map(|id| LocationRoleDto {
                location_id: *id,
                role_ids: dto.role_ids.clone(),
            })
            .collect();
        assign_location_roles(&mut tx, org, user, &ids, &dto.role_ids, &scopes).await?;
    }
    sqlx::query(r#"UPDATE organization_memberships SET "isActive"=$3 WHERE "organizationId"=$1 AND "userId"=$2"#).bind(org).bind(user).bind(dto.active).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO access_member_versions(organization_id,user_id) VALUES($1,$2) ON CONFLICT(organization_id,user_id) DO UPDATE SET revision=access_member_versions.revision+1").bind(org).bind(user).execute(&mut *tx).await?;
    keep_administrator(&mut tx, org).await?;
    audit(
        &mut tx,
        org,
        &c,
        "member.updated",
        &user.to_string(),
        before,
        json!({"active":dto.active,"roleIds":dto.role_ids,"locationIds":dto.location_ids,"locationRoles":dto.location_roles}),
    )
    .await?;
    tx.commit().await?;
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
    let org = org(&c)?;
    if !access::catalog()
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["key"] == module)
        || module == "access"
    {
        return Err(AppError::BadRequest(
            "Access administration is always enabled; choose a configurable module".into(),
        ));
    }
    let mut tx = s.db.begin().await?;
    lock(&mut tx, org).await?;
    if !dto.enabled && matches!(module.as_str(), "shifts" | "cash-registers") {
        let open:bool=sqlx::query_scalar(r#"SELECT EXISTS(SELECT 1 FROM shifts WHERE "clockOut" IS NULL AND "deletedAt" IS NULL)"#).fetch_one(&mut *tx).await?;
        if open {
            return Err(AppError::Conflict(
                "Close active shifts before disabling shifts or cash registers".into(),
            ));
        }
    }
    let before: Option<(bool, i32)> = sqlx::query_as(
        "SELECT enabled,revision FROM access_modules WHERE organization_id=$1 AND module=$2",
    )
    .bind(org)
    .bind(&module)
    .fetch_optional(&mut *tx)
    .await?;
    if before.map(|r| r.1).unwrap_or(0) != dto.expected_revision {
        return Err(AppError::Conflict(
            "Module changed. Reload before saving.".into(),
        ));
    }
    sqlx::query("INSERT INTO access_modules(organization_id,module,enabled) VALUES($1,$2,$3) ON CONFLICT(organization_id,module) DO UPDATE SET enabled=$3,revision=access_modules.revision+1").bind(org).bind(&module).bind(dto.enabled).execute(&mut *tx).await?;
    audit(
        &mut tx,
        org,
        &c,
        "module.updated",
        &module,
        json!({"enabled":before.map(|r|r.0).unwrap_or(true)}),
        json!({"enabled":dto.enabled}),
    )
    .await?;
    tx.commit().await?;
    ok(json!({"saved":true}))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewMember {
    username: String,
    password: String,
    role_ids: Vec<Uuid>,
    #[serde(default = "default_member_locations")]
    location_ids: Vec<Uuid>,
    location_roles: Option<Vec<LocationRoleDto>>,
}
fn default_member_locations() -> Vec<Uuid> {
    vec![crate::models::DEFAULT_VENUE_LOCATION_ID]
}
pub async fn create_member(
    AdminOrStaff(c): AdminOrStaff,
    State(s): State<Arc<AppState>>,
    Json(dto): Json<NewMember>,
) -> ApiResult<Value> {
    let org = org(&c)?;
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
    let hash = bcrypt::hash(&dto.password, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let mut tx = s.db.begin().await?;
    lock(&mut tx, org).await?;
    validate_roles(&mut tx, org, &dto.role_ids).await?;
    validate_locations(&mut tx, org, &dto.location_ids).await?;
    let duplicate: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE lower(username)=lower($1))")
            .bind(&username)
            .fetch_one(&mut *tx)
            .await?;
    if duplicate {
        return Err(AppError::Conflict("Username is already in use".into()));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,username,password_hash,role) VALUES($1,$2,$3,'staff')")
        .bind(id)
        .bind(&username)
        .bind(hash)
        .execute(&mut *tx)
        .await?;
    // Remove the legacy insertion trigger's default venue membership before applying the chosen organization.
    sqlx::query("DELETE FROM access_assignments WHERE user_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"DELETE FROM organization_memberships WHERE "userId"=$1 AND "organizationId"<>$2"#,
    )
    .bind(id)
    .bind(org)
    .execute(&mut *tx)
    .await?;
    sqlx::query(r#"INSERT INTO organization_memberships("organizationId","userId",role,permissions) VALUES($1,$2,'staff','[]'::jsonb) ON CONFLICT("organizationId","userId") DO NOTHING"#).bind(org).bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM access_assignments WHERE user_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for role in &dto.role_ids {
        sqlx::query(
            "INSERT INTO access_assignments(organization_id,user_id,role_id) VALUES($1,$2,$3)",
        )
        .bind(org)
        .bind(id)
        .bind(role)
        .execute(&mut *tx)
        .await?;
    }
    assign_locations(&mut tx, org, id, &dto.location_ids).await?;
    let scopes = dto.location_roles.unwrap_or_else(|| {
        dto.location_ids
            .iter()
            .map(|location_id| LocationRoleDto {
                location_id: *location_id,
                role_ids: dto.role_ids.clone(),
            })
            .collect()
    });
    assign_location_roles(&mut tx, org, id, &dto.location_ids, &dto.role_ids, &scopes).await?;
    audit(
        &mut tx,
        org,
        &c,
        "member.created",
        &id.to_string(),
        Value::Null,
        json!({"username":username,"roleIds":dto.role_ids,"locationIds":dto.location_ids}),
    )
    .await?;
    tx.commit().await?;
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
    #[tokio::test]
    #[ignore = "requires isolated migrated DATABASE_URL; fixtures roll back"]
    async fn cannot_remove_last_manager_or_assign_cross_organization_roles() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();
        let mut tx = pool.begin().await.unwrap();
        let org = crate::models::DEFAULT_ORGANIZATION_ID;
        assert!(keep_administrator(&mut tx, org).await.is_ok());
        sqlx::query("UPDATE access_roles SET permissions='[]'::jsonb WHERE organization_id=$1")
            .bind(org)
            .execute(&mut *tx)
            .await
            .unwrap();
        assert!(keep_administrator(&mut tx, org).await.is_err());
        assert!(validate_roles(&mut tx, org, &[Uuid::new_v4()])
            .await
            .is_err());
        tx.rollback().await.unwrap();
    }
}

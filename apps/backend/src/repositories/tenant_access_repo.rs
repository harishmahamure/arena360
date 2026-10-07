//! Tenant access changes replace the shared PostgreSQL advisory lock with one fenced writer.
use super::tenant_back_office::{event, now, write};
use crate::{access, error::AppError, tenancy::TenantDb};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqliteConnection;
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;
#[derive(Clone)]
pub struct TenantAccessRepository {
    db: Arc<TenantDb>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TenantRoleDto {
    pub name: String,
    pub description: String,
    pub permissions: Vec<String>,
    pub is_template: bool,
    pub expected_revision: Option<i32>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TenantLocationRoleDto {
    pub location_id: Uuid,
    pub role_ids: Vec<Uuid>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TenantMemberDto {
    pub role_ids: Vec<Uuid>,
    pub location_ids: Option<Vec<Uuid>>,
    pub location_roles: Option<Vec<TenantLocationRoleDto>>,
    pub active: bool,
    pub expected_revision: i64,
}
fn value(s: String) -> Result<Value, AppError> {
    serde_json::from_str(&s).map_err(|e| AppError::Internal(e.to_string()))
}
impl TenantAccessRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    async fn keep_manager(c: &mut SqliteConnection) -> Result<(), AppError> {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM access_assignments a JOIN access_roles r ON r.id=a.role_id JOIN users u ON u.id=a.user_id JOIN json_each(r.permissions) p ON p.value='access:manage' WHERE r.is_template=0 AND u.role IN('admin','staff') AND u.is_active=1 AND u.deleted_at IS NULL)").fetch_one(c).await?;
        if exists {
            Ok(())
        } else {
            Err(AppError::Conflict(
                "Keep at least one active member with access management permission".into(),
            ))
        }
    }
    async fn audit(
        c: &mut SqliteConnection,
        actor: Uuid,
        action: &str,
        target: &str,
        before: Value,
        after: Value,
    ) -> Result<(), AppError> {
        let ts = now()?;
        sqlx::query("INSERT INTO access_audit(actor_id,action,target_id,before_value,after_value,created_at) VALUES(?,?,?,?,?,?)").bind(actor.to_string()).bind(action).bind(target).bind(before.to_string()).bind(after.to_string()).bind(&ts).execute(&mut *c).await?;
        let mut users: Vec<String> = sqlx::query_scalar("SELECT id FROM users WHERE role IN('admin','staff') AND is_active=1 AND deleted_at IS NULL ORDER BY id").fetch_all(&mut *c).await?;
        if Uuid::parse_str(target).is_ok() {
            users.push(target.to_owned());
        }
        users.sort();
        users.dedup();
        event(
            c,
            "access",
            Uuid::now_v7(),
            "access.changed",
            None,
            false,
            json!({"action":action,"targetId":target,"userIds":users,"updatedAt":ts}),
        )
        .await
    }
    async fn role(c: &mut SqliteConnection, id: Uuid) -> Result<Option<Value>, AppError> {
        sqlx::query_scalar::<_, String>("SELECT json_object('id',id,'name',name,'description',description,'permissions',json(permissions),'isTemplate',json(CASE WHEN is_template=1 THEN 'true' ELSE 'false' END),'revision',revision) FROM access_roles WHERE id=?").bind(id.to_string()).fetch_optional(c).await?.map(value).transpose()
    }
    pub async fn save_role(
        &self,
        id: Option<Uuid>,
        dto: TenantRoleDto,
        actor: Uuid,
    ) -> Result<Uuid, AppError> {
        if dto.name.trim().is_empty()
            || dto.name.trim().chars().count() > 80
            || dto.description.len() > 1000
            || dto.permissions.len() > 100
            || dto.permissions.iter().any(|p| !access::known(p))
        {
            return Err(AppError::BadRequest(
                "Provide a role name, description, and valid permissions".into(),
            ));
        }
        if id.is_some() != dto.expected_revision.is_some() {
            return Err(AppError::BadRequest(
                "An existing role requires its revision".into(),
            ));
        }
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let id = id.unwrap_or_else(Uuid::now_v7);
                    let before = Self::role(c, id).await?;
                    let actual = before.as_ref().and_then(|v| v["revision"].as_i64());
                    if actual != dto.expected_revision.map(i64::from) {
                        return Err(AppError::Conflict("Role changed or no longer exists. Reload before saving.".into()));
                    }
                    if before.as_ref().is_some_and(|v| v["isTemplate"].as_bool() != Some(dto.is_template)) {
                        return Err(AppError::BadRequest("Role/template type cannot be changed; create a copy instead".into()));
                    }
                    let duplicate: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM access_roles WHERE lower(name)=lower(?) AND is_template=? AND id<>?)").bind(dto.name.trim()).bind(dto.is_template).bind(id.to_string()).fetch_one(&mut *c).await?;
                    if duplicate {
                        return Err(AppError::Conflict("That role or template name already exists".into()));
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
                    let revision = actual.unwrap_or(0).checked_add(1).ok_or_else(|| AppError::Conflict("Role revision exhausted".into()))?;
                    let ts = now()?;
                    sqlx::query("INSERT INTO access_roles(id,name,description,permissions,is_template,revision,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,description=excluded.description,permissions=excluded.permissions,revision=excluded.revision,updated_at=excluded.updated_at").bind(id.to_string()).bind(dto.name.trim()).bind(dto.description.trim()).bind(json!(permissions).to_string()).bind(dto.is_template).bind(revision).bind(&ts).bind(&ts).execute(&mut *c).await?;
                    Self::keep_manager(c).await?;
                    Self::audit(c, actor, "role.saved", &id.to_string(), before.unwrap_or(Value::Null), json!({"name":dto.name.trim(),"permissions":permissions,"isTemplate":dto.is_template})).await?;
                    Ok(id)
                })
            }),
        )
        .await
    }
    pub async fn delete_role(&self, id: Uuid, expected: i32, actor: Uuid) -> Result<(), AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let before = Self::role(c, id).await?.filter(|v| v["revision"].as_i64() == Some(i64::from(expected))).ok_or_else(|| AppError::Conflict("Role changed. Reload before deleting.".into()))?;
                    let assigned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM access_assignments WHERE role_id=? UNION ALL SELECT 1 FROM location_role_assignments WHERE role_id=?)").bind(id.to_string()).bind(id.to_string()).fetch_one(&mut *c).await?;
                    if assigned {
                        return Err(AppError::Conflict("Reassign every member before deleting this role".into()));
                    }
                    sqlx::query("DELETE FROM access_roles WHERE id=?").bind(id.to_string()).execute(&mut *c).await?;
                    Self::keep_manager(c).await?;
                    Self::audit(c, actor, "role.deleted", &id.to_string(), before, Value::Null).await
                })
            }),
        )
        .await
    }
    pub async fn save_module(
        &self,
        module: &str,
        enabled: bool,
        expected: i32,
        actor: Uuid,
    ) -> Result<(), AppError> {
        if module == "access"
            || !access::catalog()
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["key"] == module)
        {
            return Err(AppError::BadRequest(
                "Access administration is always enabled; choose a configurable module".into(),
            ));
        }
        let module = module.to_owned();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    if !enabled && matches!(module.as_str(), "shifts" | "cash-registers") {
                        let open: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM shifts WHERE status='active')").fetch_one(&mut *c).await?;
                        if open {
                            return Err(AppError::Conflict("Close active shifts before disabling shifts or cash registers".into()));
                        }
                    }
                    let before: Option<(bool, i32)> = sqlx::query_as("SELECT enabled,revision FROM access_modules WHERE module=?").bind(&module).fetch_optional(&mut *c).await?;
                    if before.map(|v| v.1).unwrap_or(0) != expected {
                        return Err(AppError::Conflict("Module changed. Reload before saving.".into()));
                    }
                    let next = expected.checked_add(1).ok_or_else(|| AppError::Conflict("Module revision exhausted".into()))?;
                    sqlx::query("INSERT INTO access_modules(module,enabled,revision) VALUES(?,?,?) ON CONFLICT(module) DO UPDATE SET enabled=excluded.enabled,revision=excluded.revision").bind(&module).bind(enabled).bind(next).execute(&mut *c).await?;
                    Self::audit(c, actor, "module.updated", &module, json!({"enabled":before.map(|v|v.0).unwrap_or(true)}), json!({"enabled":enabled})).await
                })
            }),
        )
        .await
    }
    async fn validate_roles(c: &mut SqliteConnection, ids: &[Uuid]) -> Result<(), AppError> {
        if ids.len() > 20 || ids.iter().collect::<HashSet<_>>().len() != ids.len() {
            return Err(AppError::BadRequest(
                "Choose at most 20 unique roles".into(),
            ));
        }
        for id in ids {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM access_roles WHERE id=? AND is_template=0)",
            )
            .bind(id.to_string())
            .fetch_one(&mut *c)
            .await?;
            if !exists {
                return Err(AppError::BadRequest(
                    "Choose roles from this tenant; templates cannot be assigned".into(),
                ));
            }
        }
        Ok(())
    }
    /// Persist a durable control-plane activation command with the local grant edit.
    /// New activation remains disabled until the control-plane commit is acknowledged.
    pub async fn save_member_assignments(
        &self,
        user: Uuid,
        dto: TenantMemberDto,
        actor: Uuid,
    ) -> Result<i64, AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let row: Option<(bool, i64, i64)> = sqlx::query_as("SELECT is_active,access_revision,identity_revision FROM users WHERE id=? AND role IN('admin','staff') AND deleted_at IS NULL").bind(user.to_string()).fetch_optional(&mut *c).await?;
                    let (active, revision, identity_revision) = row.ok_or_else(|| AppError::NotFound("Team member not found in this tenant".into()))?;
                    if revision != dto.expected_revision {
                        return Err(AppError::Conflict("Member changed. Reload before saving.".into()));
                    }
                    Self::validate_roles(c, &dto.role_ids).await?;
                    let locations = if let Some(ids) = dto.location_ids {
                        if ids.is_empty() || ids.len() > 100 || ids.iter().collect::<HashSet<_>>().len() != ids.len() {
                            return Err(AppError::BadRequest("Choose between one and 100 unique active locations".into()));
                        }
                        for id in &ids {
                            let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM venue_locations WHERE id=? AND is_active=1)").bind(id.to_string()).fetch_one(&mut *c).await?;
                            if !valid {
                                return Err(AppError::BadRequest("Choose active locations in this tenant".into()));
                            }
                        }
                        ids
                    } else {
                        sqlx::query_scalar("SELECT DISTINCT unhex(replace(location_id,'-','')) FROM location_role_assignments WHERE user_id=?").bind(user.to_string()).fetch_all(&mut *c).await?
                    };
                    let scopes = dto.location_roles.unwrap_or_else(|| locations.iter().map(|id| TenantLocationRoleDto { location_id: *id, role_ids: dto.role_ids.clone() }).collect());
                    if scopes.len() != locations.len() {
                        return Err(AppError::BadRequest("Provide roles for every selected location".into()));
                    }
                    let mut seen = HashSet::new();
                    for scope in &scopes {
                        if !locations.contains(&scope.location_id) || !seen.insert(scope.location_id) || scope.role_ids.iter().any(|r| !dto.role_ids.contains(r)) {
                            return Err(AppError::BadRequest("Location roles must match selected locations and member roles".into()));
                        }
                        Self::validate_roles(c, &scope.role_ids).await?;
                    }
                    let before_roles: Vec<String> = sqlx::query_scalar("SELECT role_id FROM access_assignments WHERE user_id=? ORDER BY role_id").bind(user.to_string()).fetch_all(&mut *c).await?;
                    sqlx::query("DELETE FROM access_assignments WHERE user_id=?").bind(user.to_string()).execute(&mut *c).await?;
                    sqlx::query("DELETE FROM location_role_assignments WHERE user_id=?").bind(user.to_string()).execute(&mut *c).await?;
                    let ts = now()?;
                    for role in &dto.role_ids {
                        sqlx::query("INSERT INTO access_assignments(user_id,role_id,created_at) VALUES(?,?,?)").bind(user.to_string()).bind(role.to_string()).bind(&ts).execute(&mut *c).await?;
                    }
                    for scope in &scopes {
                        for role in &scope.role_ids {
                            sqlx::query("INSERT INTO location_role_assignments(user_id,location_id,role_id,created_at) VALUES(?,?,?,?)").bind(user.to_string()).bind(scope.location_id.to_string()).bind(role.to_string()).bind(&ts).execute(&mut *c).await?;
                        }
                    }
                    let next = revision.checked_add(1).ok_or_else(|| AppError::Conflict("Member revision exhausted".into()))?;
                    sqlx::query("UPDATE users SET is_active=?,access_revision=?,updated_at=? WHERE id=?").bind(active && dto.active).bind(next).bind(&ts).bind(user.to_string()).execute(&mut *c).await?;
                    sqlx::query("INSERT INTO staff_membership_commands(user_id,desired_active,identity_revision,access_revision) VALUES(?,?,?,?) ON CONFLICT(user_id) DO UPDATE SET desired_active=excluded.desired_active,identity_revision=excluded.identity_revision,access_revision=excluded.access_revision").bind(user.to_string()).bind(dto.active).bind(identity_revision).bind(next).execute(&mut *c).await?;
                    Self::keep_manager(c).await?;
                    Self::audit(c, actor, "member.updated", &user.to_string(), json!({"active":active,"roleIds":before_roles}), json!({"active":dto.active,"roleIds":dto.role_ids,"locationIds":locations,"locationRoles":scopes})).await?;
                    Ok(next)
                })
            }),
        )
        .await
    }
    pub async fn snapshot(&self) -> Result<Value, AppError> {
        let pool = self.db.read_pool()?;
        let mut tx = pool.begin().await?;
        let roles: Vec<String> = sqlx::query_scalar("SELECT json_object('id',id,'name',name,'description',description,'permissions',json(permissions),'isTemplate',json(CASE WHEN is_template=1 THEN 'true' ELSE 'false' END),'revision',revision,'memberCount',(SELECT COUNT(*) FROM access_assignments a WHERE a.role_id=r.id)) FROM access_roles r ORDER BY is_template,name,id").fetch_all(&mut *tx).await?;
        let members: Vec<String> = sqlx::query_scalar("SELECT json_object('id',u.id,'username',u.username,'name',trim(COALESCE(u.first_name,'')||' '||COALESCE(u.last_name,'')),'active',json(CASE WHEN u.is_active=1 THEN 'true' ELSE 'false' END),'revision',u.access_revision,'roleIds',json((SELECT COALESCE(json_group_array(role_id),'[]') FROM(SELECT role_id FROM access_assignments WHERE user_id=u.id ORDER BY role_id))),'locationIds',json((SELECT COALESCE(json_group_array(location_id),'[]') FROM(SELECT DISTINCT location_id FROM location_role_assignments WHERE user_id=u.id ORDER BY location_id))),'locationRoles',json((SELECT COALESCE(json_group_array(json(x)),'[]') FROM(SELECT json_object('locationId',location_id,'roleIds',json_group_array(role_id)) x FROM location_role_assignments WHERE user_id=u.id GROUP BY location_id ORDER BY location_id)))) FROM users u WHERE role IN('staff','admin') AND deleted_at IS NULL ORDER BY username,id").fetch_all(&mut *tx).await?;
        let modules: Vec<String> = sqlx::query_scalar("SELECT json_object('key',module,'enabled',json(CASE WHEN enabled=1 THEN 'true' ELSE 'false' END),'revision',revision) FROM access_modules ORDER BY module").fetch_all(&mut *tx).await?;
        let audit: Vec<String> = sqlx::query_scalar("SELECT json_object('id',a.id,'action',a.action,'targetId',a.target_id,'actor',u.username,'before',json(a.before_value),'after',json(a.after_value),'at',a.created_at) FROM access_audit a LEFT JOIN users u ON u.id=a.actor_id ORDER BY a.id DESC LIMIT 100").fetch_all(&mut *tx).await?;
        let parse = |r: Vec<String>| r.into_iter().map(value).collect::<Result<Vec<_>, _>>();
        Ok(
            json!({"catalog":access::catalog(),"roles":parse(roles)?,"members":parse(members)?,"modules":parse(modules)?,"audit":parse(audit)?}),
        )
    }
}

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::Utc;
use futures::future::BoxFuture;
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Sqlite, SqliteConnection};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{UpdateUserDto, User, UserFilterDto};
use crate::tenancy::{
    format_sqlite_timestamp, write_outbox_event_on_connection, NewOutboxEvent, TenantDb,
};

type WriteOperation<T> = Box<
    dyn for<'connection> FnOnce(
            &'connection mut SqliteConnection,
        ) -> BoxFuture<'connection, Result<T, AppError>>
        + Send,
>;

const USER_SELECT: &str = r#"
SELECT unhex(replace(id, '-', '')) AS id,
       email, username, NULL AS password_hash, is_active,
       first_name, last_name, phone_number, role,
       credit_limit / 10000.0 AS credit_limit,
       NULL AS session_otp_id, NULL AS session_otp, NULL AS totp_secret,
       0 AS totp_enabled,
       unhex(replace(created_by, '-', '')) AS created_by,
       unhex(replace(updated_by, '-', '')) AS updated_by,
       created_at, updated_at, deleted_at, avatar_url
FROM users
"#;

const AUTH_USER_SELECT: &str = r#"
SELECT unhex(replace(id, '-', '')) AS id,
       email, username, password_hash, is_active,
       first_name, last_name, phone_number, role,
       credit_limit / 10000.0 AS credit_limit,
       NULL AS session_otp_id, NULL AS session_otp, NULL AS totp_secret,
       0 AS totp_enabled,
       unhex(replace(created_by, '-', '')) AS created_by,
       unhex(replace(updated_by, '-', '')) AS updated_by,
       created_at, updated_at, deleted_at, avatar_url
FROM users
"#;

#[derive(Debug, Clone)]
pub struct TenantCreatePlayer {
    pub username: String,
    pub password_hash: String,
    pub phone_number: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub actor_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantLocationRoleGrant {
    pub system_key: String,
    pub location_id: Uuid,
}

/// A deliberately secret-free control-plane membership projection.
#[derive(Debug, Clone)]
pub struct TenantStaffProjection {
    pub user_id: Uuid,
    pub email: Option<String>,
    pub username: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone_number: Option<String>,
    pub avatar_url: Option<String>,
    pub role: String,
    pub permissions: Vec<String>,
    pub is_active: bool,
    pub deleted: bool,
    pub member_revision: i64,
    pub global_access_role_system_keys: Vec<String>,
    pub location_grants: Vec<TenantLocationRoleGrant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaffProjectionResult {
    Applied,
    Unchanged,
}

#[derive(sqlx::FromRow)]
struct StoredStaffProjection {
    role: String,
    member_revision: i64,
    created_at: String,
    deleted_at: Option<String>,
    email: Option<String>,
    username: String,
    first_name: Option<String>,
    last_name: Option<String>,
    phone_number: Option<String>,
    avatar_url: Option<String>,
    permissions: String,
    is_active: bool,
}

#[derive(Clone)]
pub struct TenantUserRepository {
    db: Arc<TenantDb>,
}

impl TenantUserRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }

    /// Global profile refreshes preserve tenant-owned role and venue assignments.
    /// Only first hydration and a global membership role change install baseline roles.
    pub async fn project_identity(
        &self,
        p: TenantStaffProjection,
        revision: i64,
    ) -> Result<StaffProjectionResult, AppError> {
        if revision <= 0 || !matches!(p.role.as_str(), "admin" | "staff") {
            return Err(AppError::BadRequest(
                "Invalid staff identity projection".into(),
            ));
        }
        // The writer rechecks this state under its transaction. This read only
        // avoids taking the writer (and renewing activity) for unchanged polls.
        let current: Option<(String, i64)> = sqlx::query_as("SELECT role,identity_revision FROM users WHERE id=?")
            .bind(p.user_id.to_string()).fetch_optional(&self.db.background_read_pool()?).await?;
        if current.as_ref().is_some_and(|v| v.0 != "player" && v.1 >= revision) {
            return Ok(StaffProjectionResult::Unchanged);
        }
        write(&self.db, Box::new(move |c| Box::pin(async move {
            let old: Option<(String,i64)> = sqlx::query_as("SELECT role,identity_revision FROM users WHERE id=?")
                .bind(p.user_id.to_string()).fetch_optional(&mut *c).await?;
            if old.as_ref().is_some_and(|v| v.0 == "player") {
                return Err(AppError::Conflict("Control-plane staff identity collides with a tenant player".into()));
            }
            if old.as_ref().is_some_and(|v| v.1 >= revision) {
                return Ok(StaffProjectionResult::Unchanged);
            }
            let ts = now()?;
            let pending: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM staff_membership_commands WHERE user_id=?)").bind(p.user_id.to_string()).fetch_one(&mut *c).await?;
            if pending { return Ok(StaffProjectionResult::Unchanged); }
            let active = p.is_active && !p.deleted;
            if !active && old.is_some() {
                // Revocation must succeed even if the new global profile name
                // collides with a tenant-local player created while disconnected.
                sqlx::query("UPDATE users SET is_active=0,deleted_at=?,identity_revision=?,updated_at=? WHERE id=?")
                    .bind(p.deleted.then_some(ts.clone())).bind(revision).bind(&ts).bind(p.user_id.to_string()).execute(&mut *c).await?;
                super::tenant_back_office::event(c,"user",p.user_id,"user.updated",None,false,
                    json!({"id":p.user_id,"isActive":false,"deleted":p.deleted,"identityRevision":revision,"updatedAt":ts})).await?;
                return Ok(StaffProjectionResult::Applied);
            }
            let result = sqlx::query("INSERT INTO users(id,email,username,password_hash,first_name,last_name,phone_number,avatar_url,role,permissions,is_active,credit_limit,created_at,updated_at,deleted_at,identity_revision) VALUES(?,?,?,NULL,?,?,?,?,?,'[]',?,0,?,?,?,?) ON CONFLICT(id) DO UPDATE SET email=excluded.email,username=excluded.username,password_hash=NULL,first_name=excluded.first_name,last_name=excluded.last_name,phone_number=excluded.phone_number,avatar_url=excluded.avatar_url,role=excluded.role,is_active=excluded.is_active,updated_at=excluded.updated_at,deleted_at=excluded.deleted_at,identity_revision=excluded.identity_revision")
                .bind(p.user_id.to_string()).bind(&p.email).bind(&p.username).bind(&p.first_name).bind(&p.last_name).bind(&p.phone_number).bind(&p.avatar_url).bind(&p.role).bind(active).bind(&ts).bind(&ts).bind(p.deleted.then_some(ts.clone())).bind(revision).execute(&mut *c).await;
            map_unique(result, &p.username)?;
            if old.as_ref().map(|v| v.0.as_str()) != Some(p.role.as_str()) {
                if old.is_some() {
                    sqlx::query("UPDATE users SET access_revision=access_revision+1 WHERE id=?").bind(p.user_id.to_string()).execute(&mut *c).await?;
                }
                clear_assignments(c,p.user_id).await?;
                if !p.deleted {
                    let role: Option<String> = sqlx::query_scalar("SELECT id FROM access_roles WHERE system_key=? AND is_template=0")
                        .bind(&p.role).fetch_optional(&mut *c).await?;
                    let role = role.ok_or_else(|| AppError::Conflict("Tenant baseline role missing".into()))?;
                    sqlx::query("INSERT INTO access_assignments(user_id,role_id,created_at) VALUES(?,?,?)")
                        .bind(p.user_id.to_string()).bind(role).bind(&ts).execute(&mut *c).await?;
                }
            }
            super::tenant_back_office::event(c,"user",p.user_id,"user.updated",None,false,
                json!({"id":p.user_id,"username":p.username,"firstName":p.first_name,"lastName":p.last_name,"role":p.role,"isActive":active,"deleted":p.deleted,"identityRevision":revision,"updatedAt":ts})).await?;
            Ok(StaffProjectionResult::Applied)
        }))).await
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, AppError> {
        Ok(sqlx::query_as::<_, User>(&format!(
            "{USER_SELECT} WHERE id = ? AND deleted_at IS NULL"
        ))
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?)
    }

    pub async fn list(&self, filters: &UserFilterDto) -> Result<PaginationResult<User>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let mut query =
            QueryBuilder::<Sqlite>::new(format!("{USER_SELECT} WHERE deleted_at IS NULL"));
        apply_filters(&mut query, filters);
        let sort = match filters.sort_by.as_deref() {
            Some("username") => "username",
            Some("phoneNumber") => "phone_number",
            Some("role") => "role",
            _ => "created_at",
        };
        let direction = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        query
            .push(format!(
                " ORDER BY {sort} {direction}, id {direction} LIMIT "
            ))
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind((page - 1) * limit);
        let users = query
            .build_query_as::<User>()
            .fetch_all(&self.db.read_pool()?)
            .await?;

        let mut count =
            QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL");
        apply_filters(&mut count, filters);
        let total = count
            .build_query_scalar()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(users, total, page, limit))
    }

    pub async fn username_exists(
        &self,
        username: &str,
        exclude_id: Option<Uuid>,
    ) -> Result<bool, AppError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users WHERE lower(username)=lower(?) \
             AND (? IS NULL OR id<>?) AND deleted_at IS NULL)",
        )
        .bind(username)
        .bind(exclude_id.map(|id| id.to_string()))
        .bind(exclude_id.map(|id| id.to_string()))
        .fetch_one(&self.db.read_pool()?)
        .await?)
    }

    pub async fn find_player_for_auth(&self, username: &str) -> Result<Option<User>, AppError> {
        Ok(sqlx::query_as::<_, User>(&format!(
            "{AUTH_USER_SELECT} WHERE lower(username)=lower(?) AND role='player' \
             AND is_active=1 AND deleted_at IS NULL"
        ))
        .bind(username)
        .fetch_optional(&self.db.read_pool()?)
        .await?)
    }

    pub async fn create_player(&self, values: TenantCreatePlayer) -> Result<User, AppError> {
        let id = Uuid::now_v7();
        let at = now()?;
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    let result = sqlx::query(
                        "INSERT INTO users(id,username,password_hash,phone_number,first_name,\
                         last_name,role,permissions,member_revision,is_active,credit_limit,\
                         created_by,updated_by,created_at,updated_at) \
                         VALUES(?,?,?,?,?,?,'player','[]',1,1,0,?,?,?,?)",
                    )
                    .bind(id.to_string())
                    .bind(&values.username)
                    .bind(&values.password_hash)
                    .bind(&values.phone_number)
                    .bind(&values.first_name)
                    .bind(&values.last_name)
                    .bind(values.actor_id.map(|value| value.to_string()))
                    .bind(values.actor_id.map(|value| value.to_string()))
                    .bind(&at)
                    .bind(&at)
                    .execute(&mut *connection)
                    .await;
                    map_unique(result, &values.username)?;
                    user_event(
                        connection,
                        id,
                        "user.created",
                        false,
                        public_payload(
                            id,
                            &values.username,
                            values.first_name.as_deref(),
                            values.last_name.as_deref(),
                            "player",
                            true,
                            0,
                            Some(&at),
                            &at,
                            false,
                        ),
                    )
                    .await
                })
            }),
        )
        .await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::Internal("Created tenant user disappeared".into()))
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdateUserDto,
        actor_id: Option<Uuid>,
    ) -> Result<User, AppError> {
        let at = now()?;
        let values = (
            dto.username.clone(),
            dto.phone_number.clone(),
            dto.first_name.clone(),
            dto.last_name.clone(),
            dto.is_active,
        );
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    let result = sqlx::query(
                        "UPDATE users SET username=COALESCE(?,username),\
                         phone_number=COALESCE(?,phone_number),first_name=COALESCE(?,first_name),\
                         last_name=COALESCE(?,last_name),is_active=COALESCE(?,is_active),\
                         updated_by=COALESCE(?,updated_by),updated_at=? \
                         WHERE id=? AND role='player' AND deleted_at IS NULL",
                    )
                    .bind(&values.0)
                    .bind(&values.1)
                    .bind(&values.2)
                    .bind(&values.3)
                    .bind(values.4)
                    .bind(actor_id.map(|value| value.to_string()))
                    .bind(&at)
                    .bind(id.to_string())
                    .execute(&mut *connection)
                    .await;
                    let result = map_unique(result, values.0.as_deref().unwrap_or("user"))?;
                    if result.rows_affected() == 0 {
                        return Err(AppError::NotFound(format!("User with ID {id} not found")));
                    }
                    let row: PublicEventRow = sqlx::query_as(
                        "SELECT username,first_name,last_name,role,is_active,credit_limit,\
                         created_at FROM users WHERE id=?",
                    )
                    .bind(id.to_string())
                    .fetch_one(&mut *connection)
                    .await?;
                    user_event(
                        connection,
                        id,
                        "user.updated",
                        false,
                        row.payload(id, &at, false),
                    )
                    .await
                })
            }),
        )
        .await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("User with ID {id} not found")))
    }

    pub async fn set_avatar(&self, id: Uuid, avatar_url: Option<&str>) -> Result<User, AppError> {
        self.mutate_player(id, "avatar_url", avatar_url.map(str::to_owned))
            .await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("User with ID {id} not found")))
    }

    pub async fn update_password(&self, id: Uuid, password_hash: &str) -> Result<(), AppError> {
        self.mutate_player(id, "password_hash", Some(password_hash.to_owned()))
            .await
    }

    async fn mutate_player(
        &self,
        id: Uuid,
        column: &'static str,
        value: Option<String>,
    ) -> Result<(), AppError> {
        let at = now()?;
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    let sql = format!(
                        "UPDATE users SET {column}=?,updated_at=? \
                         WHERE id=? AND role='player' AND deleted_at IS NULL"
                    );
                    let result = sqlx::query(&sql)
                        .bind(value)
                        .bind(&at)
                        .bind(id.to_string())
                        .execute(&mut *connection)
                        .await?;
                    if result.rows_affected() == 0 {
                        return Err(AppError::NotFound(format!("User with ID {id} not found")));
                    }
                    let row: PublicEventRow = sqlx::query_as(
                        "SELECT username,first_name,last_name,role,is_active,credit_limit,\
                         created_at FROM users WHERE id=?",
                    )
                    .bind(id.to_string())
                    .fetch_one(&mut *connection)
                    .await?;
                    user_event(
                        connection,
                        id,
                        "user.updated",
                        false,
                        row.payload(id, &at, false),
                    )
                    .await
                })
            }),
        )
        .await
    }

    pub async fn soft_delete(&self, id: Uuid) -> Result<(), AppError> {
        let at = now()?;
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    let row: Option<PublicEventRow> = sqlx::query_as(
                        "UPDATE users SET deleted_at=?,is_active=0,updated_at=? \
                         WHERE id=? AND role='player' AND deleted_at IS NULL \
                         RETURNING username,first_name,last_name,role,is_active,credit_limit,created_at",
                    )
                    .bind(&at)
                    .bind(&at)
                    .bind(id.to_string())
                    .fetch_optional(&mut *connection)
                    .await?;
                    let row = row.ok_or_else(|| {
                        AppError::NotFound(format!("User with ID {id} not found"))
                    })?;
                    user_event(
                        connection,
                        id,
                        "user.deleted",
                        true,
                        row.payload(id, &at, true),
                    )
                    .await
                })
            }),
        )
        .await
    }

    pub async fn require_active_player(&self, id: Uuid) -> Result<User, AppError> {
        self.require_active_role(id, "player").await
    }

    pub async fn require_active_staff(&self, id: Uuid) -> Result<User, AppError> {
        let user = self
            .find_by_id(id)
            .await?
            .filter(|user| {
                user.is_active && matches!(user.role.as_deref(), Some("staff" | "admin"))
            })
            .ok_or_else(|| AppError::NotFound(format!("Active staff user {id} not found")))?;
        Ok(user)
    }

    async fn require_active_role(&self, id: Uuid, role: &str) -> Result<User, AppError> {
        self.find_by_id(id)
            .await?
            .filter(|user| user.is_active && user.role.as_deref() == Some(role))
            .ok_or_else(|| AppError::NotFound(format!("Active {role} user {id} not found")))
    }

    pub async fn project_staff(
        &self,
        projection: TenantStaffProjection,
    ) -> Result<StaffProjectionResult, AppError> {
        if !matches!(projection.role.as_str(), "staff" | "admin") {
            return Err(AppError::BadRequest(
                "Projected membership role must be staff or admin".into(),
            ));
        }
        if projection.member_revision <= 0 {
            return Err(AppError::BadRequest(
                "Projected membership revision must be positive".into(),
            ));
        }
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move { apply_staff_projection(connection, projection).await })
            }),
        )
        .await
    }

    pub async fn location_ids_for_permission(
        &self,
        user_id: Uuid,
        permission: &str,
    ) -> Result<Vec<Uuid>, AppError> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT l.id FROM venue_locations l JOIN users u ON u.id=? \
             WHERE u.is_active=1 AND u.deleted_at IS NULL AND l.is_active=1 \
               AND COALESCE((SELECT enabled FROM access_modules WHERE module=?),1)=1 \
               AND (u.role='admin' OR (u.role='staff' AND EXISTS(\
                 SELECT 1 FROM location_role_assignments a \
                 JOIN access_roles r ON r.id=a.role_id \
                 JOIN json_each(r.permissions) p ON p.value=? \
                 WHERE a.user_id=u.id AND a.location_id=l.id))) \
             ORDER BY l.id",
        )
        .bind(user_id.to_string())
        .bind(permission.split(':').next().unwrap_or(permission))
        .bind(permission)
        .fetch_all(&self.db.read_pool()?)
        .await?;
        rows.into_iter()
            .map(|id| {
                Uuid::parse_str(&id)
                    .map_err(|error| AppError::Internal(format!("invalid location UUID: {error}")))
            })
            .collect()
    }
}

async fn apply_staff_projection(
    connection: &mut SqliteConnection,
    mut projection: TenantStaffProjection,
) -> Result<StaffProjectionResult, AppError> {
    canonicalize_projection(&mut projection);
    let existing: Option<StoredStaffProjection> = sqlx::query_as(
        "SELECT role,member_revision,created_at,deleted_at,email,username,first_name,last_name,\
         phone_number,avatar_url,permissions,is_active FROM users WHERE id=?",
    )
    .bind(projection.user_id.to_string())
    .fetch_optional(&mut *connection)
    .await?;
    if existing.as_ref().is_some_and(|row| row.role == "player") {
        return Err(AppError::Conflict(
            "Control-plane staff identity collides with a tenant player".into(),
        ));
    }
    if let Some(row) = &existing {
        if projection.member_revision < row.member_revision {
            return Err(AppError::Conflict(
                "Projected membership revision is stale".into(),
            ));
        }
        if projection.member_revision == row.member_revision {
            let matches = if !projection.is_active || projection.deleted {
                revocation_matches(connection, row, &projection).await?
            } else {
                validate_location_role_subset(&projection)?;
                resolve_roles(connection, &projection.global_access_role_system_keys).await?;
                let location_keys = projection
                    .location_grants
                    .iter()
                    .map(|grant| grant.system_key.clone())
                    .collect::<Vec<_>>();
                resolve_roles(connection, &location_keys).await?;
                validate_locations(connection, &projection.location_grants).await?;
                projection_matches(connection, row, &projection).await?
            };
            return if matches {
                Ok(StaffProjectionResult::Unchanged)
            } else {
                Err(AppError::Conflict(
                    "Projected membership revision has divergent content".into(),
                ))
            };
        }
    }

    let at = now()?;
    if !projection.is_active || projection.deleted {
        return match existing.as_ref() {
            Some(_) => {
                revoke_staff_projection(connection, existing.as_ref(), &projection, &at).await
            }
            None => hydrate_revoked_projection(connection, &projection, &at).await,
        };
    }

    validate_location_role_subset(&projection)?;
    let global_roles =
        resolve_roles(connection, &projection.global_access_role_system_keys).await?;
    let location_keys = projection
        .location_grants
        .iter()
        .map(|grant| grant.system_key.clone())
        .collect::<Vec<_>>();
    let location_roles = resolve_roles(connection, &location_keys).await?;
    validate_locations(connection, &projection.location_grants).await?;
    let permissions = serde_json::to_string(&projection.permissions)
        .map_err(|error| AppError::BadRequest(format!("invalid permissions: {error}")))?;
    let deleted_at = projection.deleted.then_some(at.clone());
    let event_type = if projection.deleted {
        "user.deleted"
    } else if existing.is_some() {
        "user.updated"
    } else {
        "user.created"
    };
    let created_at = existing
        .as_ref()
        .map(|row| row.created_at.clone())
        .unwrap_or_else(|| at.clone());

    let result = sqlx::query(
        "INSERT INTO users(id,email,username,password_hash,first_name,last_name,phone_number,\
         avatar_url,role,permissions,member_revision,is_active,credit_limit,created_at,updated_at,\
         deleted_at) VALUES(?,?,?,NULL,?,?,?,?,?,?,?, ?,0,?,?,?) \
         ON CONFLICT(id) DO UPDATE SET email=excluded.email,username=excluded.username,\
         password_hash=NULL,first_name=excluded.first_name,last_name=excluded.last_name,\
         phone_number=excluded.phone_number,avatar_url=excluded.avatar_url,role=excluded.role,\
         permissions=excluded.permissions,member_revision=excluded.member_revision,\
         is_active=excluded.is_active,updated_at=excluded.updated_at,deleted_at=excluded.deleted_at",
    )
    .bind(projection.user_id.to_string())
    .bind(&projection.email)
    .bind(&projection.username)
    .bind(&projection.first_name)
    .bind(&projection.last_name)
    .bind(&projection.phone_number)
    .bind(&projection.avatar_url)
    .bind(&projection.role)
    .bind(permissions)
    .bind(projection.member_revision)
    .bind(projection.is_active && !projection.deleted)
    .bind(&created_at)
    .bind(&at)
    .bind(&deleted_at)
    .execute(&mut *connection)
    .await;
    map_unique(result, &projection.username)?;

    clear_assignments(connection, projection.user_id).await?;
    if projection.is_active && !projection.deleted {
        for role_id in global_roles.values() {
            sqlx::query("INSERT INTO access_assignments(user_id,role_id,created_at) VALUES(?,?,?)")
                .bind(projection.user_id.to_string())
                .bind(role_id.to_string())
                .bind(&at)
                .execute(&mut *connection)
                .await?;
        }
        for grant in &projection.location_grants {
            let role_id = location_roles.get(&grant.system_key).ok_or_else(|| {
                AppError::Conflict(format!("Unknown access role '{}'", grant.system_key))
            })?;
            sqlx::query(
                "INSERT OR IGNORE INTO location_role_assignments(user_id,location_id,role_id,created_at) \
                 VALUES(?,?,?,?)",
            )
            .bind(projection.user_id.to_string())
            .bind(grant.location_id.to_string())
            .bind(role_id.to_string())
            .bind(&at)
            .execute(&mut *connection)
            .await?;
        }
    }
    user_event(
        connection,
        projection.user_id,
        event_type,
        projection.deleted,
        public_payload(
            projection.user_id,
            &projection.username,
            projection.first_name.as_deref(),
            projection.last_name.as_deref(),
            &projection.role,
            projection.is_active && !projection.deleted,
            0,
            Some(&created_at),
            &at,
            projection.deleted,
        ),
    )
    .await?;
    Ok(StaffProjectionResult::Applied)
}

fn canonicalize_projection(projection: &mut TenantStaffProjection) {
    projection.permissions.sort();
    projection.permissions.dedup();
    projection.global_access_role_system_keys.sort();
    projection.global_access_role_system_keys.dedup();
    projection.location_grants.sort_by(|left, right| {
        (left.location_id, left.system_key.as_str())
            .cmp(&(right.location_id, right.system_key.as_str()))
    });
    projection.location_grants.dedup();
    if projection.deleted {
        projection.is_active = false;
    }
}

fn validate_location_role_subset(projection: &TenantStaffProjection) -> Result<(), AppError> {
    if let Some(grant) = projection.location_grants.iter().find(|grant| {
        projection
            .global_access_role_system_keys
            .binary_search(&grant.system_key)
            .is_err()
    }) {
        return Err(AppError::Conflict(format!(
            "Location access role '{}' is not globally assigned",
            grant.system_key
        )));
    }
    Ok(())
}

async fn revocation_matches(
    connection: &mut SqliteConnection,
    stored: &StoredStaffProjection,
    projection: &TenantStaffProjection,
) -> Result<bool, AppError> {
    let assignment_counts: (i64, i64) = sqlx::query_as(
        "SELECT \
         (SELECT COUNT(*) FROM access_assignments WHERE user_id=?),\
         (SELECT COUNT(*) FROM location_role_assignments WHERE user_id=?)",
    )
    .bind(projection.user_id.to_string())
    .bind(projection.user_id.to_string())
    .fetch_one(&mut *connection)
    .await?;
    Ok(!stored.is_active
        && stored.deleted_at.is_some() == projection.deleted
        && assignment_counts == (0, 0))
}

async fn hydrate_revoked_projection(
    connection: &mut SqliteConnection,
    projection: &TenantStaffProjection,
    at: &str,
) -> Result<StaffProjectionResult, AppError> {
    let permissions = serde_json::to_string(&projection.permissions)
        .map_err(|error| AppError::BadRequest(format!("invalid permissions: {error}")))?;
    let deleted_at = projection.deleted.then_some(at);
    let result = sqlx::query(
        "INSERT INTO users(id,email,username,password_hash,first_name,last_name,phone_number,\
         avatar_url,role,permissions,member_revision,is_active,credit_limit,created_at,updated_at,\
         deleted_at) VALUES(?,?,?,NULL,?,?,?,?,?,?,?,0,0,?,?,?)",
    )
    .bind(projection.user_id.to_string())
    .bind(&projection.email)
    .bind(&projection.username)
    .bind(&projection.first_name)
    .bind(&projection.last_name)
    .bind(&projection.phone_number)
    .bind(&projection.avatar_url)
    .bind(&projection.role)
    .bind(permissions)
    .bind(projection.member_revision)
    .bind(at)
    .bind(at)
    .bind(deleted_at)
    .execute(&mut *connection)
    .await;
    map_unique(result, &projection.username)?;
    user_event(
        connection,
        projection.user_id,
        if projection.deleted {
            "user.deleted"
        } else {
            "user.created"
        },
        projection.deleted,
        public_payload(
            projection.user_id,
            &projection.username,
            projection.first_name.as_deref(),
            projection.last_name.as_deref(),
            &projection.role,
            false,
            0,
            Some(at),
            at,
            projection.deleted,
        ),
    )
    .await?;
    Ok(StaffProjectionResult::Applied)
}

async fn projection_matches(
    connection: &mut SqliteConnection,
    stored: &StoredStaffProjection,
    projection: &TenantStaffProjection,
) -> Result<bool, AppError> {
    let mut stored_permissions: Vec<String> = serde_json::from_str(&stored.permissions)
        .map_err(|error| AppError::Internal(format!("invalid stored permissions: {error}")))?;
    stored_permissions.sort();
    stored_permissions.dedup();
    let global_keys: Vec<String> = sqlx::query_scalar(
        "SELECT COALESCE(r.system_key,'') FROM access_assignments a \
         JOIN access_roles r ON r.id=a.role_id WHERE a.user_id=? \
         ORDER BY COALESCE(r.system_key,'')",
    )
    .bind(projection.user_id.to_string())
    .fetch_all(&mut *connection)
    .await?;
    let location_keys: Vec<(String, String)> = sqlx::query_as(
        "SELECT a.location_id,COALESCE(r.system_key,'') FROM location_role_assignments a \
         JOIN access_roles r ON r.id=a.role_id WHERE a.user_id=? \
         ORDER BY a.location_id,COALESCE(r.system_key,'')",
    )
    .bind(projection.user_id.to_string())
    .fetch_all(&mut *connection)
    .await?;
    let incoming_locations = projection
        .location_grants
        .iter()
        .map(|grant| (grant.location_id.to_string(), grant.system_key.clone()))
        .collect::<Vec<_>>();

    Ok(stored.email == projection.email
        && stored.username == projection.username
        && stored.first_name == projection.first_name
        && stored.last_name == projection.last_name
        && stored.phone_number == projection.phone_number
        && stored.avatar_url == projection.avatar_url
        && stored.role == projection.role
        && stored_permissions == projection.permissions
        && stored.is_active == projection.is_active
        && stored.deleted_at.is_some() == projection.deleted
        && global_keys == projection.global_access_role_system_keys
        && location_keys == incoming_locations)
}

async fn revoke_staff_projection(
    connection: &mut SqliteConnection,
    existing: Option<&StoredStaffProjection>,
    projection: &TenantStaffProjection,
    at: &str,
) -> Result<StaffProjectionResult, AppError> {
    let row = existing.ok_or_else(|| {
        AppError::Internal("staff revocation requires an existing projected user".into())
    })?;
    let deleted_at = projection.deleted.then_some(at);
    sqlx::query(
        "UPDATE users SET member_revision=?,is_active=0,deleted_at=?,updated_at=? WHERE id=?",
    )
    .bind(projection.member_revision)
    .bind(deleted_at)
    .bind(at)
    .bind(projection.user_id.to_string())
    .execute(&mut *connection)
    .await?;
    clear_assignments(connection, projection.user_id).await?;
    user_event(
        connection,
        projection.user_id,
        if projection.deleted {
            "user.deleted"
        } else {
            "user.updated"
        },
        projection.deleted,
        public_payload(
            projection.user_id,
            &row.username,
            row.first_name.as_deref(),
            row.last_name.as_deref(),
            &row.role,
            false,
            0,
            Some(&row.created_at),
            at,
            projection.deleted,
        ),
    )
    .await?;
    Ok(StaffProjectionResult::Applied)
}

async fn clear_assignments(
    connection: &mut SqliteConnection,
    user_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM location_role_assignments WHERE user_id=?")
        .bind(user_id.to_string())
        .execute(&mut *connection)
        .await?;
    sqlx::query("DELETE FROM access_assignments WHERE user_id=?")
        .bind(user_id.to_string())
        .execute(&mut *connection)
        .await?;
    Ok(())
}

async fn resolve_roles(
    connection: &mut SqliteConnection,
    keys: &[String],
) -> Result<BTreeMap<String, Uuid>, AppError> {
    let mut roles = BTreeMap::new();
    for key in keys {
        if roles.contains_key(key) {
            continue;
        }
        let id: Option<String> =
            sqlx::query_scalar("SELECT id FROM access_roles WHERE system_key=? AND is_template=0")
                .bind(key)
                .fetch_optional(&mut *connection)
                .await?;
        let id = id.ok_or_else(|| AppError::Conflict(format!("Unknown access role '{key}'")))?;
        let id = Uuid::parse_str(&id)
            .map_err(|error| AppError::Internal(format!("invalid access role UUID: {error}")))?;
        roles.insert(key.clone(), id);
    }
    Ok(roles)
}

async fn validate_locations(
    connection: &mut SqliteConnection,
    grants: &[TenantLocationRoleGrant],
) -> Result<(), AppError> {
    for grant in grants {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM venue_locations WHERE id=? AND is_active=1)",
        )
        .bind(grant.location_id.to_string())
        .fetch_one(&mut *connection)
        .await?;
        if !exists {
            return Err(AppError::Conflict(format!(
                "Unknown or inactive venue location '{}'",
                grant.location_id
            )));
        }
    }
    Ok(())
}

fn apply_filters<'a>(query: &mut QueryBuilder<'a, Sqlite>, filters: &'a UserFilterDto) {
    if let Some(username) = &filters.username {
        query
            .push(" AND instr(lower(username),lower(")
            .push_bind(username)
            .push("))>0");
    }
    if let Some(phone) = &filters.phone_number {
        query
            .push(" AND instr(lower(COALESCE(phone_number,'')),lower(")
            .push_bind(phone)
            .push("))>0");
    }
    if let Some(role) = &filters.role {
        query.push(" AND role=").push_bind(role);
    }
    if let Some(active) = filters.is_active {
        query.push(" AND is_active=").push_bind(active == 1);
    }
}

async fn write<T: Send + 'static>(
    db: &TenantDb,
    operation: WriteOperation<T>,
) -> Result<T, AppError> {
    db.with_immediate_writer(operation).await
}

fn now() -> Result<String, AppError> {
    format_sqlite_timestamp(&Utc::now())
        .map_err(|error| AppError::Internal(format!("format tenant timestamp: {error}")))
}

fn map_unique<T>(result: Result<T, sqlx::Error>, username: &str) -> Result<T, AppError> {
    result.map_err(|error| {
        if matches!(&error, sqlx::Error::Database(database) if database.is_unique_violation()) {
            AppError::Conflict(format!(
                "User with username or email '{username}' already exists"
            ))
        } else {
            AppError::Database(error)
        }
    })
}

#[derive(sqlx::FromRow)]
struct PublicEventRow {
    username: String,
    first_name: Option<String>,
    last_name: Option<String>,
    role: String,
    is_active: bool,
    credit_limit: i64,
    created_at: String,
}

impl PublicEventRow {
    fn payload(&self, id: Uuid, updated_at: &str, deleted: bool) -> Value {
        public_payload(
            id,
            &self.username,
            self.first_name.as_deref(),
            self.last_name.as_deref(),
            &self.role,
            self.is_active,
            self.credit_limit,
            Some(&self.created_at),
            updated_at,
            deleted,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn public_payload(
    id: Uuid,
    username: &str,
    first_name: Option<&str>,
    last_name: Option<&str>,
    role: &str,
    is_active: bool,
    credit_limit: i64,
    created_at: Option<&str>,
    updated_at: &str,
    deleted: bool,
) -> Value {
    let mut payload = json!({
        "id": id,
        "username": username,
        "firstName": first_name,
        "lastName": last_name,
        "role": role,
        "isActive": is_active,
        "creditLimit": credit_limit as f64 / 10000.0,
        "updatedAt": updated_at,
    });
    if let Some(created_at) = created_at {
        payload["createdAt"] = json!(created_at);
    }
    if deleted {
        payload["deleted"] = json!(true);
    }
    payload
}

async fn user_event(
    connection: &mut SqliteConnection,
    id: Uuid,
    event_type: &str,
    deleted: bool,
    payload: Value,
) -> Result<(), AppError> {
    write_outbox_event_on_connection(
        connection,
        NewOutboxEvent {
            location_id: None,
            aggregate_type: "user".into(),
            aggregate_id: id,
            event_type: event_type.into(),
            schema_version: 1,
            deleted,
            payload,
        },
    )
    .await?;
    Ok(())
}

//! Control-plane changes are projected by the owning cell. Business requests read
//! only the local projection; an unavailable control plane does not block them.
use crate::{
    error::AppError,
    repositories::{TenantStaffProjection, TenantUserRepository},
    tenancy::{TenantDb, TenantDbManager},
};
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct Identity {
    user_id: Uuid,
    revision: i64,
    email: Option<String>,
    username: String,
    first_name: Option<String>,
    last_name: Option<String>,
    phone_number: Option<String>,
    avatar_url: Option<String>,
    role: String,
    is_active: bool,
    deleted: bool,
}

pub async fn sync_tenant(pool: &PgPool, db: Arc<TenantDb>) -> Result<(), AppError> {
    flush_membership_commands(pool, db.clone()).await?;
    let rows: Vec<Identity> = sqlx::query_as(
        "SELECT p.user_id,p.revision,u.email,COALESCE(u.username,'revoked_'||p.user_id::text) AS username,u.first_name,u.last_name,u.phone_number,u.avatar_url,COALESCE(m.role,'staff') AS role,COALESCE(u.is_active AND u.deleted_at IS NULL AND m.is_active AND t.state NOT IN('DELETED','FAILED'),false) AS is_active,(u.id IS NULL OR u.deleted_at IS NOT NULL OR m.id IS NULL) AS deleted FROM staff_projection_changes p JOIN tenants t ON t.id=p.tenant_id LEFT JOIN users u ON u.id=p.user_id LEFT JOIN organization_memberships m ON m.tenant_id=p.tenant_id AND m.user_id=p.user_id WHERE p.tenant_id=$1 ORDER BY p.revision")
        .bind(db.tenant_id()).fetch_all(pool).await?;
    let repo = TenantUserRepository::new(db);
    let mut failure = None;
    for row in rows {
        let user_id = row.user_id;
        let result = repo
            .project_identity(
                TenantStaffProjection {
                    user_id: row.user_id,
                    email: row.email,
                    username: row.username,
                    first_name: row.first_name,
                    last_name: row.last_name,
                    phone_number: row.phone_number,
                    avatar_url: row.avatar_url,
                    role: row.role,
                    is_active: row.is_active,
                    deleted: row.deleted,
                    permissions: vec![],
                    member_revision: 0,
                    global_access_role_system_keys: vec![],
                    location_grants: vec![],
                },
                row.revision,
            )
            .await;
        if let Err(error) = result {
            tracing::warn!(%user_id,%error,"staff profile projection failed");
            if failure.is_none() {
                failure = Some(error);
            }
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

async fn flush_membership_commands(pool: &PgPool, db: Arc<TenantDb>) -> Result<(), AppError> {
    let commands: Vec<(String,bool,i64,i64)> = sqlx::query_as("SELECT user_id,desired_active,access_revision,identity_revision FROM staff_membership_commands ORDER BY user_id")
        .fetch_all(&db.read_pool()?).await?;
    for (user, active, version, expected) in commands {
        let user_id = Uuid::parse_str(&user)
            .map_err(|_| AppError::Internal("Invalid membership command identity".into()))?;
        let mut tx = pool.begin().await?;
        let owned: Option<Uuid> = sqlx::query_scalar("SELECT t.id FROM tenants t JOIN tenant_leases l ON l.tenant_id=t.id AND l.owner_cell=t.owner_cell AND l.ownership_generation=t.ownership_generation WHERE t.id=$1 AND t.ownership_generation=$2 AND l.expires_at>clock_timestamp() AND t.state='ACTIVE' FOR UPDATE OF t,l")
            .bind(db.tenant_id()).bind(db.ownership_generation()).fetch_optional(&mut *tx).await?;
        if owned.is_none() {
            return Err(AppError::Forbidden(
                "Membership command requires current tenant ownership".into(),
            ));
        }
        let receipt: Option<(i64,bool)> = sqlx::query_as("SELECT control_revision,conflicted FROM staff_membership_command_receipts WHERE tenant_id=$1 AND user_id=$2 AND access_revision=$3 AND ownership_generation=$4")
            .bind(db.tenant_id()).bind(user_id).bind(version).bind(db.ownership_generation()).fetch_optional(&mut *tx).await?;
        let (revision, conflicted) = if let Some(receipt) = receipt {
            receipt
        } else {
            let _: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM organization_memberships WHERE tenant_id=$1 AND user_id=$2 FOR UPDATE")
                .bind(db.tenant_id()).bind(user_id).fetch_optional(&mut *tx).await?;
            let (actual,current_active): (i64,bool) = sqlx::query_as("SELECT p.revision,COALESCE(m.is_active,false) FROM staff_projection_changes p LEFT JOIN organization_memberships m ON m.tenant_id=p.tenant_id AND m.user_id=p.user_id WHERE p.tenant_id=$1 AND p.user_id=$2 FOR UPDATE OF p")
                .bind(db.tenant_id()).bind(user_id).fetch_one(&mut *tx).await?;
            let conflicted = active && actual != expected && !current_active;
            // A global revocation/profile change wins over an older local request.
            let revision = if actual == expected || (!active && current_active) {
                sqlx::query("UPDATE organization_memberships SET is_active=$3,updated_at=now() WHERE tenant_id=$1 AND user_id=$2")
                    .bind(db.tenant_id()).bind(user_id).bind(active).execute(&mut *tx).await?;
                sqlx::query_scalar("SELECT revision FROM staff_projection_changes WHERE tenant_id=$1 AND user_id=$2")
                    .bind(db.tenant_id()).bind(user_id).fetch_one(&mut *tx).await?
            } else {
                actual
            };
            sqlx::query("INSERT INTO staff_membership_command_receipts(tenant_id,user_id,access_revision,ownership_generation,control_revision,conflicted) VALUES($1,$2,$3,$4,$5,$6)")
                .bind(db.tenant_id()).bind(user_id).bind(version).bind(db.ownership_generation()).bind(revision).bind(conflicted).execute(&mut *tx).await?;
            if active && !conflicted {
                sqlx::query(
                    "DELETE FROM pending_staff_creations WHERE tenant_id=$1 AND user_id=$2",
                )
                .bind(db.tenant_id())
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
            }
            (revision, conflicted)
        };
        tx.commit().await?;
        db.with_immediate_writer(move |c| Box::pin(async move {
            let current: Option<i64> = sqlx::query_scalar("SELECT access_revision FROM staff_membership_commands WHERE user_id=?")
                .bind(&user).fetch_optional(&mut *c).await?;
            if current == Some(version) {
                // Ignore a snapshot read before this global commit, even if it arrives later.
                sqlx::query("UPDATE users SET identity_revision=max(identity_revision,?) WHERE id=?")
                    .bind(revision-1).bind(&user).execute(&mut *c).await?;
                sqlx::query("DELETE FROM staff_membership_commands WHERE user_id=? AND access_revision=?")
                    .bind(user).bind(version).execute(&mut *c).await?;
                if conflicted {
                    crate::tenancy::write_outbox_event_on_connection(c,crate::tenancy::NewOutboxEvent {
                        aggregate_type:"access".into(),aggregate_id:user_id,event_type:"access.membership_conflict".into(),
                        payload:serde_json::json!({"userId":user_id,"accessRevision":version,"controlRevision":revision}),
                        location_id:None,schema_version:1,deleted:false
                    }).await?;
                }
            }
            Ok(())
        })).await?;
    }
    Ok(())
}

pub fn spawn(pool: PgPool, manager: Arc<TenantDbManager>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            for db in manager.open_handles().await {
                let tenant = db.tenant_id();
                if let Err(error) = sync_tenant(&pool, db).await {
                    tracing::warn!(%tenant,%error,"staff identity projection delayed");
                }
            }
        }
    });
}

use chrono::Utc;
use gaming_cafe_api::{
    error::AppError,
    models::*,
    repositories::{
        TenantAccessRepository, TenantConfigRepository, TenantMemberDto,
        TenantNotificationRepository, TenantSettingsRepository,
    },
    services::notification_service::{Recipients, RecordNotification},
    tenancy::{tenant_path, TenantDb, TenantDbConfig, TenantDbManager, TenantLease},
};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
    time::Duration,
};
use uuid::Uuid;
#[derive(Default)]
struct Lease {
    generations: RwLock<HashMap<Uuid, i64>>,
}

impl TenantLease for Lease {
    fn writable_generation(&self, tenant_id: Uuid) -> Result<i64, AppError> {
        self.generations
            .read()
            .unwrap()
            .get(&tenant_id)
            .copied()
            .ok_or_else(|| AppError::Forbidden("tenant lease is not writable".into()))
    }

    fn ensure_writable(&self, tenant_id: Uuid, generation: i64) -> Result<(), AppError> {
        if self.writable_generation(tenant_id)? == generation {
            Ok(())
        } else {
            Err(AppError::Forbidden(
                "tenant lease generation changed".into(),
            ))
        }
    }
}

struct Fixture {
    root: PathBuf,
    db: Arc<TenantDb>,
    lease: Arc<Lease>,
}

impl Fixture {
    async fn new() -> Self {
        let root = std::env::temp_dir().join(format!("arena360-catalog-repo-{}", Uuid::now_v7()));
        let tenant_id = Uuid::now_v7();
        let path = tenant_path(&root, tenant_id);
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        gaming_cafe_api::tenancy::migrate(&pool).await.unwrap();
        sqlx::query("INSERT INTO tenant_runtime(singleton,timezone) VALUES(1,'Asia/Kolkata')")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
        let lease = Arc::new(Lease::default());
        lease.generations.write().unwrap().insert(tenant_id, 1);
        let manager = TenantDbManager::new(
            TenantDbConfig {
                root: root.clone(),
                read_connections: 2,
                busy_timeout: Duration::from_millis(250),
                idle_timeout: Duration::from_secs(60),
                reaper_interval: Duration::from_secs(1),
            },
            lease.clone(),
        )
        .unwrap();
        let db = manager.open(tenant_id).await.unwrap();
        Self { root, db, lease }
    }

    async fn location(&self, slug: &str) -> Uuid {
        let id = Uuid::now_v7();
        let slug = slug.to_owned();
        let timestamp = gaming_cafe_api::time::format_sqlite_timestamp(&Utc::now()).unwrap();
        self.db
            .with_writer(|connection| {
                Box::pin(async move {
                    sqlx::query("INSERT INTO venue_locations(id,slug,name,created_at,updated_at) VALUES (?,?,?,?,?)")
                        .bind(id.to_string())
                        .bind(&slug)
                        .bind(&slug)
                        .bind(&timestamp)
                        .bind(&timestamp)
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .await
            .unwrap();
        id
    }

    async fn outbox_count(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
            .fetch_one(&self.db.read_pool().unwrap())
            .await
            .unwrap()
    }

    async fn close(self) {
        self.db.close().await.unwrap();
        tokio::fs::remove_dir_all(self.root).await.unwrap();
    }
}

impl Fixture {
    async fn staff(&self) -> Uuid {
        let id = Uuid::now_v7();
        self.db.with_immediate_writer(move|c|Box::pin(async move{
   let ts=gaming_cafe_api::tenancy::format_sqlite_timestamp(&Utc::now()).unwrap();
   sqlx::query("INSERT INTO users(id,username,role,created_at,updated_at) VALUES(?,?,'staff',?,?)").bind(id.to_string()).bind(id.to_string()).bind(&ts).bind(&ts).execute(c).await?;Ok(())
  })).await.unwrap();
        id
    }
    async fn scalar(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sql)
            .fetch_one(&self.db.read_pool().unwrap())
            .await
            .unwrap()
    }
}

fn dto<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> T {
    serde_json::from_value(v).unwrap()
}
#[tokio::test]
async fn settings_revision_races_history_and_scope_are_safe() {
    let f = Fixture::new().await;
    let venue = f.location("main").await;
    let user = f.staff().await;
    let settings = TenantSettingsRepository::new(f.db.clone());
    let tenant = f.db.tenant_id();
    let a: UpsertSettingOverrideDto = dto(
        json!({"value":"Asia/Kolkata","reason":"fixture","expectedRevision":0,"locationId":venue}),
    );
    let b: UpsertSettingOverrideDto =
        dto(json!({"value":"UTC","reason":"fixture","expectedRevision":0,"locationId":venue}));
    let (a, b) = tokio::join!(
        settings.upsert_override(tenant, "general.timezone", &a, user, Some("a"), false),
        settings.upsert_override(tenant, "general.timezone", &b, user, Some("b"), false)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let current = settings
        .find_override(tenant, Some(venue), "general.timezone")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.revision, 1);
    assert_eq!(current.created_by, Some(user));
    assert_eq!(f.scalar("SELECT COUNT(*) FROM setting_revisions").await, 1);
    assert!(settings
        .delete_override(
            tenant,
            Some(venue),
            "general.timezone",
            Some(0),
            "reset",
            user,
            None,
            None
        )
        .await
        .is_err());
    settings
        .delete_override(
            tenant,
            Some(venue),
            "general.timezone",
            Some(1),
            "reset",
            user,
            None,
            None,
        )
        .await
        .unwrap();
    let next = settings
        .upsert_override(
            tenant,
            "general.timezone",
            &dto(
                json!({"value":"UTC","reason":"recreate","expectedRevision":0,"locationId":venue}),
            ),
            user,
            None,
            false,
        )
        .await
        .unwrap();
    assert_eq!(next.revision, 3);
    let history = settings
        .history(
            tenant,
            &dto(json!({"locationId":venue,"key":"general.timezone"})),
        )
        .await
        .unwrap();
    assert_eq!(history.len(), 3);
    assert_eq!(history[1].operation, "delete");
    assert_eq!(
        settings.latest_revision(tenant).await.unwrap(),
        history[0].id
    );
    assert!(settings
        .upsert_override(
            Uuid::now_v7(),
            "general.timezone",
            &dto(json!({"value":"UTC","reason":"wrong tenant"})),
            user,
            None,
            false
        )
        .await
        .is_err());
    assert!(settings
        .upsert_override(
            tenant,
            "general.timezone",
            &dto(json!({"value":"UTC","reason":"invalid scope","locationId":Uuid::now_v7()})),
            user,
            None,
            false
        )
        .await
        .is_err());
    let config = TenantConfigRepository::new(f.db.clone());
    let row = config
        .upsert(
            "custom.value",
            "custom",
            &dto(json!({"value":{"x":1},"description":"first"})),
            user,
        )
        .await
        .unwrap();
    let updated = config
        .upsert(
            "custom.value",
            "other",
            &dto(json!({"value":{"x":2}})),
            user,
        )
        .await
        .unwrap();
    assert_eq!(row.id, updated.id);
    assert_eq!(updated.category, "custom");
    assert_eq!(updated.description.as_deref(), Some("first"));
    assert_eq!(
        config
            .find_all(&dto(json!({"key":"VALUE"})))
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(config.delete_by_key("custom.value").await.unwrap());
    assert!(!config.delete_by_key("custom.value").await.unwrap());
    settings
        .upsert_override(
            tenant,
            "pricing.currency",
            &dto(json!({"value":"USD","reason":"global value"})),
            user,
            None,
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        config
            .find_by_key("pricing.currency")
            .await
            .unwrap()
            .unwrap()
            .value,
        json!("USD")
    );
    settings
        .delete_override(
            tenant,
            None,
            "pricing.currency",
            Some(1),
            "reset",
            user,
            None,
            Some(&json!("INR")),
        )
        .await
        .unwrap();
    assert_eq!(
        config
            .find_by_key("pricing.currency")
            .await
            .unwrap()
            .unwrap()
            .value,
        json!("INR")
    );
    assert!(settings
        .find_override(tenant, None, "pricing.currency")
        .await
        .unwrap()
        .is_none());
    f.close().await;
}
fn notification(kind: &str, actor: Uuid) -> RecordNotification {
    RecordNotification {
        kind: kind.into(),
        title: "Order".into(),
        summary: Some("summary".into()),
        payload: json!({"x":1}),
        actor_user_id: Some(actor),
        entity_type: Some("order".into()),
        entity_id: Some(Uuid::now_v7()),
        recipients: Recipients::AllStaff,
    }
}
#[tokio::test]
async fn notifications_are_atomic_scoped_and_retained_by_policy() {
    let f = Fixture::new().await;
    let staff = f.staff().await;
    let other = f.staff().await;
    let repo = TenantNotificationRepository::new(f.db.clone());
    repo.record(notification(activity_kind::TRANSACTION_SALE, staff))
        .await
        .unwrap();
    assert_eq!(repo.unread_count(staff, false).await.unwrap(), 0);
    let activity = repo
        .record(notification(activity_kind::KIOSK_ORDER_PLACED, staff))
        .await
        .unwrap();
    assert_eq!(repo.unread_count(staff, true).await.unwrap(), 1);
    assert_eq!(f.scalar("SELECT COUNT(*) FROM user_notifications").await, 2);
    let inbox = repo
        .list_notifications(staff, &NotificationFilterDto::default())
        .await
        .unwrap();
    assert_eq!(inbox.data[0].activity_id, activity.id);
    assert_eq!(inbox.data[0].payload, json!({"x":1}));
    assert!(!repo.mark_read(inbox.data[0].id, other).await.unwrap());
    assert!(repo.mark_read(inbox.data[0].id, staff).await.unwrap());
    assert!(!repo.mark_read(inbox.data[0].id, staff).await.unwrap());
    assert_eq!(repo.mark_all_read(other).await.unwrap(), 1);
    assert_eq!(
        repo.list_activity_log(other, false, &ActivityLogFilterDto::default())
            .await
            .unwrap()
            .total,
        1
    );
    assert_eq!(
        repo.list_activity_log(staff, true, &ActivityLogFilterDto::default())
            .await
            .unwrap()
            .total,
        2
    );
    let before = f.outbox_count().await;
    assert!(repo
        .record(notification("invalid_kind", staff))
        .await
        .is_err());
    assert_eq!(f.outbox_count().await, before);
    assert_eq!(f.scalar("SELECT COUNT(*) FROM activity_log").await, 2);
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE users SET is_active=0 WHERE id=?")
                .bind(other.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert_eq!(repo.cleanup_notifications(7).await.unwrap(), 1);
    assert_eq!(f.scalar("SELECT COUNT(*) FROM activity_log").await, 2);
    f.lease.generations.write().unwrap().clear();
    assert!(repo
        .record(notification(activity_kind::KIOSK_ORDER_PLACED, staff))
        .await
        .is_err());
    f.close().await;
}
#[tokio::test]
async fn access_edits_preserve_manager_revision_and_scopes() {
    let f = Fixture::new().await;
    let venue = f.location("main").await;
    let staff = f.staff().await;
    let member = f.staff().await;
    let role = Uuid::now_v7();
    let ts = gaming_cafe_api::time::format_sqlite_timestamp(&Utc::now()).unwrap();
    f.db.with_immediate_writer(move|c|Box::pin(async move{sqlx::query("INSERT INTO access_roles(id,name,permissions,created_at,updated_at) VALUES(?,'Manager','[\"access:manage\",\"settings:read\"]',?,?)").bind(role.to_string()).bind(&ts).bind(&ts).execute(&mut *c).await?;sqlx::query("INSERT INTO access_assignments(user_id,role_id,created_at) VALUES(?,?,?)").bind(staff.to_string()).bind(role.to_string()).bind(&ts).execute(c).await?;Ok(())})).await.unwrap();
    let access = TenantAccessRepository::new(f.db.clone());
    let before = f.outbox_count().await;
    assert!(access.save_role(Some(role),dto(json!({"name":"Manager","description":"","permissions":[],"isTemplate":false,"expectedRevision":1})),staff).await.is_err());
    assert_eq!(f.outbox_count().await, before);
    let read_role=access.save_role(None,dto(json!({"name":"Read","description":"","permissions":["settings:read"],"isTemplate":false})),staff).await.unwrap();
    let m: TenantMemberDto = dto(
        json!({"roleIds":[read_role],"locationIds":[venue],"active":true,"expectedRevision":0}),
    );
    assert_eq!(
        access
            .save_member_assignments(member, m.clone(), staff)
            .await
            .unwrap(),
        1
    );
    assert_eq!(f.scalar("SELECT COUNT(*) FROM staff_membership_commands").await,1);
    assert!(access
        .save_member_assignments(member, m, staff)
        .await
        .is_err());
    let settings = TenantSettingsRepository::new(f.db.clone());
    settings
        .ensure_access(f.db.tenant_id(), member, "settings:read")
        .await
        .unwrap();
    settings
        .ensure_location_permission(f.db.tenant_id(), venue, member, "settings:read")
        .await
        .unwrap();
    assert_eq!(
        settings
            .list_locations(f.db.tenant_id(), member)
            .await
            .unwrap()
            .len(),
        1
    );
    access
        .save_module("settings", false, 0, staff)
        .await
        .unwrap();
    assert!(settings
        .ensure_access(f.db.tenant_id(), member, "settings:read")
        .await
        .is_err());
    assert!(settings
        .ensure_location_permission(f.db.tenant_id(), venue, member, "settings:read")
        .await
        .is_err());
    assert!(access
        .save_module("settings", true, 0, staff)
        .await
        .is_err());
    assert!(access.delete_role(read_role, 1, staff).await.is_err());
    assert!(access
        .save_member_assignments(
            staff,
            dto(json!({"roleIds":[],"active":false,"expectedRevision":0})),
            staff
        )
        .await
        .is_err());
    assert_eq!(
        f.scalar(&format!("SELECT is_active FROM users WHERE id='{staff}'"))
            .await,
        1
    );
    let snapshot = access.snapshot().await.unwrap();
    assert_eq!(snapshot["roles"].as_array().unwrap().len(), 2);
    assert_eq!(snapshot["members"].as_array().unwrap().len(), 2);
    assert_eq!(snapshot["modules"][0]["enabled"], false);
    assert_eq!(snapshot["audit"].as_array().unwrap().len(), 3);
    f.close().await;
}

#[tokio::test]
async fn tenant_location_scope_uses_current_grants_and_rejects_foreign_claims() {
    use gaming_cafe_api::{access::scope::LocationScope, dto::JwtUserClaims};
    let f = Fixture::new().await;
    let user = f.staff().await;
    let venue = f.location("permitted").await;
    let other = f.location("unassigned").await;
    let role = Uuid::now_v7();
    f.db.with_immediate_writer(move |c| Box::pin(async move {
        let at = gaming_cafe_api::tenancy::format_sqlite_timestamp(&Utc::now()).unwrap();
        sqlx::query("INSERT INTO access_roles(id,name,permissions,created_at,updated_at) VALUES(?,'Scoped reader','[\"products:read\"]',?,?)")
            .bind(role.to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
        sqlx::query("INSERT INTO access_assignments(user_id,role_id,created_at) VALUES(?,?,?)")
            .bind(user.to_string()).bind(role.to_string()).bind(&at).execute(&mut *c).await?;
        sqlx::query("INSERT INTO location_role_assignments(user_id,location_id,role_id,created_at) VALUES(?,?,?,?)")
            .bind(user.to_string()).bind(venue.to_string()).bind(role.to_string()).bind(&at).execute(c).await?;
        Ok(())
    })).await.unwrap();
    let mut claims: JwtUserClaims =
        dto(json!({"sub":user,"userId":user,"tenantId":f.db.tenant_id(),
        "roles":["staff"],"permissions":["products:read"],"allowedTenants":[f.db.tenant_id()],
        "iss":"gamezone","aud":"gamezone","appId":"admin","orgIds":[f.db.tenant_id()]}));
    let scope = LocationScope::resolve_tenant(f.db.clone(), &claims, "products:read", None)
        .await
        .unwrap();
    assert_eq!(scope.locations, vec![venue]);
    assert!(
        LocationScope::resolve_tenant(f.db.clone(), &claims, "products:read", Some(other))
            .await
            .is_err()
    );
    assert!(
        LocationScope::resolve_tenant(f.db.clone(), &claims, "products:write", Some(venue))
            .await
            .is_err()
    );
    assert_eq!(f.db.timezone().await.unwrap(), "Asia/Kolkata");
    claims.tenantId = Uuid::now_v7().to_string();
    assert!(
        LocationScope::resolve_tenant(f.db.clone(), &claims, "products:read", None)
            .await
            .is_err()
    );
    claims.tenantId = f.db.tenant_id().to_string();
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE users SET is_active=0 WHERE id=?")
                .bind(user.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(
        LocationScope::resolve_tenant(f.db.clone(), &claims, "products:read", None)
            .await
            .is_err()
    );
    f.close().await;
}

#[tokio::test]
async fn location_edits_are_atomic_and_keep_live_venues_active() {
    let f = Fixture::new().await;
    let repo = TenantSettingsRepository::new(f.db.clone());
    let tenant = f.db.tenant_id();
    let details = || SaveVenueLocationDto {
        slug: "north".into(),
        name: "North hall".into(),
        timezone: "Europe/London".into(),
        currency: "GBP".into(),
        is_active: None,
    };
    let venue = repo.save_location(tenant, None, details()).await.unwrap();
    assert_eq!(venue.timezone, "Europe/London");
    assert_eq!(venue.currency, "GBP");
    let before = f.outbox_count().await;
    assert!(repo.save_location(tenant, None, details()).await.is_err());
    assert_eq!(f.outbox_count().await, before);
    let user = f.staff().await;
    let shift = gaming_cafe_api::repositories::TenantShiftRepository::new(f.db.clone());
    shift.create(user, venue.id, None, user).await.unwrap();
    let mut edit = details();
    edit.is_active = Some(false);
    assert!(repo
        .save_location(tenant, Some(venue.id), edit)
        .await
        .is_err());
    assert!(repo.list_managed_locations(tenant).await.unwrap()[0].is_active);
    assert_eq!(f.outbox_count().await, before + 1);
    assert!(repo
        .save_location(Uuid::now_v7(), None, details())
        .await
        .is_err());
    let mut invalid = details();
    invalid.timezone = "Invalid/Zone".into();
    assert!(repo
        .save_location(tenant, Some(venue.id), invalid)
        .await
        .is_err());
    f.close().await;
}

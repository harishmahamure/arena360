use gaming_cafe_api::{
    error::AppError,
    tenancy::{tenant_path, TenantDb, TenantDbConfig, TenantDbManager, TenantLease},
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{path::PathBuf, sync::Arc, time::Duration};
use uuid::Uuid;

struct Lease(Uuid, std::sync::atomic::AtomicBool);
impl TenantLease for Lease {
    fn writable_generation(&self, tenant: Uuid) -> Result<i64, AppError> {
        if tenant == self.0 && !self.1.load(std::sync::atomic::Ordering::Acquire) {
            Ok(1)
        } else {
            Err(AppError::Forbidden("foreign test tenant".into()))
        }
    }
    fn ensure_writable(&self, tenant: Uuid, generation: i64) -> Result<(), AppError> {
        if self.writable_generation(tenant)? == generation {
            Ok(())
        } else {
            Err(AppError::Forbidden("stale test generation".into()))
        }
    }
}

/// An isolated, migrated tenant file with a lease scoped to exactly that tenant.
pub struct TenantFixture {
    pub db: Arc<TenantDb>,
    pub manager: Arc<TenantDbManager>,
    root: PathBuf,
    lease: Arc<Lease>,
}
impl TenantFixture {
    pub async fn new() -> Self {
        Self::new_with_idle(Duration::from_secs(60)).await
    }
    pub fn revoke(&self) {
        self.lease.1.store(true, std::sync::atomic::Ordering::Release);
    }
    pub async fn new_with_idle(idle_timeout: Duration) -> Self {
        let tenant = Uuid::now_v7();
        let root = std::env::temp_dir().join(format!("arena360-service-test-{tenant}"));
        let path = tenant_path(&root, tenant);
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
        sqlx::query("INSERT INTO tenant_runtime(singleton,timezone) VALUES(1,'UTC')")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
        let lease = Arc::new(Lease(tenant, std::sync::atomic::AtomicBool::new(false)));
        let manager = Arc::new(
            TenantDbManager::new(
                TenantDbConfig {
                    root: root.clone(),
                    read_connections: 2,
                    busy_timeout: Duration::from_millis(500),
                    idle_timeout,
                    reaper_interval: Duration::from_secs(1),
                },
                lease.clone(),
            )
            .unwrap(),
        );
        let db = manager.open(tenant).await.unwrap();
        Self { db, root, manager, lease }
    }
    pub async fn player(&self, username: &str) -> Uuid {
        gaming_cafe_api::repositories::TenantUserRepository::new(self.db.clone())
            .create_player(gaming_cafe_api::repositories::TenantCreatePlayer {
                username: username.into(),
                password_hash: bcrypt::hash("initial-password", 4).unwrap(),
                phone_number: "9999999999".into(),
                first_name: None,
                last_name: None,
                actor_id: None,
            })
            .await
            .unwrap()
            .id
    }
    pub async fn plan_transaction(&self, player: Uuid, plan: Uuid, venue: Uuid) -> Uuid {
        let transaction = Uuid::now_v7();
        self.db.with_immediate_writer(move |c| Box::pin(async move {
            let at = gaming_cafe_api::time::format_sqlite_timestamp(&chrono::Utc::now()).unwrap();
            sqlx::query("INSERT INTO transactions(id,player_id,plan_id,location_id,transaction_type,amount,paid_amount,cash_amount,payment_method,payment_status,transaction_date,created_at,updated_at) VALUES(?,?,?,?,'plan_purchase',10000,10000,10000,'cash','completed',?,?,?)")
                .bind(transaction.to_string()).bind(player.to_string()).bind(plan.to_string()).bind(venue.to_string()).bind(&at).bind(&at).bind(&at).execute(c).await?;
            Ok(())
        })).await.unwrap();
        transaction
    }
    pub async fn close(self) {
        self.db.close().await.unwrap();
        tokio::fs::remove_dir_all(self.root).await.unwrap();
    }
}

#[derive(Default)]
pub struct MemoryCache(std::sync::RwLock<std::collections::HashMap<String, serde_json::Value>>);
#[async_trait::async_trait]
impl gaming_cafe_api::cache::CacheService for MemoryCache {
    async fn get_value(&self, key: &str) -> Result<Option<serde_json::Value>, AppError> {
        Ok(self.0.read().unwrap().get(key).cloned())
    }
    async fn set_value(
        &self,
        key: &str,
        value: &serde_json::Value,
        _: Duration,
    ) -> Result<(), AppError> {
        self.0.write().unwrap().insert(key.into(), value.clone());
        Ok(())
    }
    async fn delete(&self, keys: &[&str]) -> Result<(), AppError> {
        let mut values = self.0.write().unwrap();
        for key in keys {
            values.remove(*key);
        }
        Ok(())
    }
    async fn invalidate_prefix(&self, prefix: &str) -> Result<(), AppError> {
        self.0
            .write()
            .unwrap()
            .retain(|key, _| !key.starts_with(prefix));
        Ok(())
    }
    async fn publish_invalidation(&self, _: &[String]) -> Result<(), AppError> {
        Ok(())
    }
    async fn consume_ip_token(
        &self,
        _: &str,
        _: u32,
        _: Duration,
    ) -> Result<Option<gaming_cafe_api::cache::RateLimitDecision>, AppError> {
        Ok(None)
    }
    fn is_available(&self) -> bool {
        true
    }
}

pub struct SessionFixture {
    pub tenant: TenantFixture,
    pub venue: Uuid,
    pub player: Uuid,
    pub plan: Uuid,
    pub balance: Uuid,
    pub device: Uuid,
    pub sessions: gaming_cafe_api::services::SessionService,
    pub balances: Arc<gaming_cafe_api::services::BalanceService>,
    pub cache: Arc<MemoryCache>,
}
impl SessionFixture {
    pub async fn new() -> Self {
        use gaming_cafe_api::{
            cache::CacheService,
            models::PurchaseBalanceDto,
            repositories::TenantDeviceRepository,
            services::{
                BalanceService, ConfigService, DeviceService, EventService, PlanService,
                PricingPolicyService,
            },
            sse::Broadcaster,
        };
        let tenant = TenantFixture::new().await;
        let venue = Uuid::now_v7();
        tenant.db.with_immediate_writer(move |c| Box::pin(async move {
            let at = gaming_cafe_api::time::format_sqlite_timestamp(&chrono::Utc::now()).unwrap();
            sqlx::query("INSERT INTO venue_locations(id,slug,name,created_at,updated_at) VALUES(?,'main','Main',?,?)")
                .bind(venue.to_string()).bind(&at).bind(&at).execute(c).await?; Ok(())
        })).await.unwrap();
        let player = tenant.player("session-player").await;
        let cache = Arc::new(MemoryCache::default());
        let settings = Arc::new(ConfigService::new(cache.clone(), "UTC".into()));
        let plan = PlanService::new(settings.clone()).create_tenant(tenant.db.clone(), vec![venue],
            serde_json::from_value(serde_json::json!({"name":"Two hours","price":100,"planType":"time_based","validityDays":30,"timeCredits":120,"deviceType":"PC","deviceSubType":"HIGH_END_PCS"})).unwrap(),None).await.unwrap().id;
        let balances = Arc::new(BalanceService::new(cache.clone()));
        let balance = balances
            .purchase_or_recharge_tenant(
                tenant.db.clone(),
                PurchaseBalanceDto {
                    player_id: player,
                    plan_id: plan,
                    transaction_id: Some(tenant.plan_transaction(player, plan, venue).await),
                },
                None,
            )
            .await
            .unwrap()
            .id;
        let device = TenantDeviceRepository::new(tenant.db.clone()).create(
            &serde_json::from_value(serde_json::json!({"name":"PC-01","locationId":venue,"deviceType":"PC","deviceSubType":"HIGH_END_PCS","registrationStatus":"registered"})).unwrap(),None).await.unwrap().id;
        let events = EventService::new(Broadcaster::new(16));
        let devices = DeviceService::new(events.clone(), cache.clone());
        let sessions = gaming_cafe_api::services::SessionService::new(
            devices,
            balances.clone(),
            events,
            settings,
            PricingPolicyService::new(),
            cache.clone(),
        );
        // Fixture setup can populate raw wallet caches; each assertion starts deliberately.
        cache.invalidate_prefix("").await.unwrap();
        Self {
            tenant,
            venue,
            player,
            plan,
            balance,
            device,
            sessions,
            balances,
            cache,
        }
    }
    pub async fn start(
        &self,
        at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> gaming_cafe_api::models::UsageSession {
        self.sessions
            .start_tenant(
                self.tenant.db.clone(),
                gaming_cafe_api::models::CreateSessionDto {
                    balance_id: self.balance,
                    device_id: self.device,
                    shift_id: None,
                    start_time: at,
                },
                self.player,
                None,
            )
            .await
            .unwrap()
    }
    pub async fn recharge(&self) -> gaming_cafe_api::models::PlayerPlanBalance {
        self.balances
            .purchase_or_recharge_tenant(
                self.tenant.db.clone(),
                gaming_cafe_api::models::PurchaseBalanceDto {
                    player_id: self.player,
                    plan_id: self.plan,
                    transaction_id: Some(
                        self.tenant
                            .plan_transaction(self.player, self.plan, self.venue)
                            .await,
                    ),
                },
                None,
            )
            .await
            .unwrap()
    }
    pub async fn close(self) {
        self.tenant.close().await;
    }
}
impl TenantFixture {
    pub async fn venue(&self, slug: &str) -> Uuid {
        gaming_cafe_api::repositories::TenantSettingsRepository::new(self.db.clone())
            .save_location(
                self.db.tenant_id(),
                None,
                serde_json::from_value(
                    serde_json::json!({"slug":slug,"name":slug,"timezone":"UTC","currency":"INR"}),
                )
                .unwrap(),
            )
            .await
            .unwrap()
            .id
    }
    pub async fn staff(&self, venue: Option<Uuid>, permissions: Vec<String>) -> Uuid {
        let user = Uuid::now_v7();
        let role = Uuid::now_v7();
        self.db.with_immediate_writer(move|c|Box::pin(async move {
            let at=gaming_cafe_api::time::format_sqlite_timestamp(&chrono::Utc::now()).unwrap();
            sqlx::query("INSERT INTO users(id,username,role,created_at,updated_at) VALUES(?,?,'staff',?,?)").bind(user.to_string()).bind(user.to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
            sqlx::query("INSERT INTO access_roles(id,name,permissions,created_at,updated_at) VALUES(?,?,?,?,?)").bind(role.to_string()).bind(role.to_string()).bind(serde_json::to_string(&permissions).unwrap()).bind(&at).bind(&at).execute(&mut *c).await?;
            sqlx::query("INSERT INTO access_assignments(user_id,role_id,created_at) VALUES(?,?,?)").bind(user.to_string()).bind(role.to_string()).bind(&at).execute(&mut *c).await?;
            if let Some(venue)=venue {sqlx::query("INSERT INTO location_role_assignments(user_id,location_id,role_id,created_at) VALUES(?,?,?,?)").bind(user.to_string()).bind(venue.to_string()).bind(role.to_string()).bind(&at).execute(c).await?;}
            Ok(())
        })).await.unwrap();
        user
    }
}

/// Explicit settings keep HTTP acceptance tests independent of developer environment and services.
pub fn settings() -> gaming_cafe_api::config::Settings {
    gaming_cafe_api::config::Settings {
        roles: gaming_cafe_api::config::Roles {
            control: false,
            cell: true,
            router: false,
        },
        cell_id: None,
        tenant_data_dir: std::env::temp_dir().join(format!("arena360-http-{}", Uuid::now_v7())),
        database_url: "postgres://unused:unused@127.0.0.1:1/unused".into(),
        control_database_url: None,
        database_listener_url: "postgres://unused:unused@127.0.0.1:1/unused".into(),
        database_min_connections: 0,
        database_max_connections: 2,
        database_acquire_timeout_seconds: 1,
        database_idle_timeout_seconds: 60,
        database_max_lifetime_seconds: 600,
        redis_url: None,
        jwt_secret: "arena360-test-secret-at-least-thirty-two-characters".into(),
        jwt_access_expiration: "15m".into(),
        jwt_player_expiration: "24h".into(),
        jwt_device_expiration: "365d".into(),
        bcrypt_salt_rounds: 4,
        port: 0,
        cafe_timezone: "UTC".into(),
        zeptomail_token: None,
        legacy_rest_enabled: true,
        trusted_proxy_cidrs: vec![],
        max_concurrent_requests: 64,
    }
}
impl SessionFixture {
    pub async fn app(&self) -> Arc<gaming_cafe_api::app::AppState> {
        let mut state = gaming_cafe_api::app::build_state_with_settings(Arc::new(settings())).await;
        Arc::get_mut(&mut state).unwrap().tenant_dbs = Some(self.tenant.manager.clone());
        state
    }
    pub async fn second_device(&self) -> Uuid {
        gaming_cafe_api::repositories::TenantDeviceRepository::new(self.tenant.db.clone()).create(
            &serde_json::from_value(serde_json::json!({"name":"PC-02","locationId":self.venue,"deviceType":"PC","deviceSubType":"HIGH_END_PCS","registrationStatus":"registered"})).unwrap(),None).await.unwrap().id
    }
    pub async fn registration(&self, value: &str) {
        let id = self.device.to_string();
        let value = value.to_owned();
        self.tenant
            .db
            .with_immediate_writer(move |c| {
                Box::pin(async move {
                    sqlx::query("UPDATE devices SET registration_status=? WHERE id=?")
                        .bind(value)
                        .bind(id)
                        .execute(c)
                        .await?;
                    Ok(())
                })
            })
            .await
            .unwrap();
    }
}
pub async fn request(
    app: axum::Router,
    method: &str,
    path: &str,
    device: Option<&str>,
    player: Option<&str>,
    body: serde_json::Value,
) -> (axum::http::StatusCode, serde_json::Value) {
    use tower::ServiceExt;
    let mut request = axum::http::Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = device {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    if let Some(token) = player {
        request = request.header("x-player-token", token);
    }
    let response = app
        .oneshot(
            request
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| serde_json::json!({"raw":String::from_utf8_lossy(&bytes)}));
    (status, body)
}

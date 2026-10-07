use gaming_cafe_api::{
    error::AppError,
    tenancy::{tenant_path, TenantDb, TenantDbConfig, TenantDbManager, TenantLease},
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{path::PathBuf, sync::Arc, time::Duration};
use uuid::Uuid;

struct Lease(Uuid);
impl TenantLease for Lease {
    fn writable_generation(&self, tenant: Uuid) -> Result<i64, AppError> {
        if tenant == self.0 {
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
    root: PathBuf,
}
impl TenantFixture {
    pub async fn new() -> Self {
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
        let manager = TenantDbManager::new(
            TenantDbConfig {
                root: root.clone(),
                read_connections: 2,
                busy_timeout: Duration::from_millis(500),
                idle_timeout: Duration::from_secs(60),
                reaper_interval: Duration::from_secs(1),
            },
            Arc::new(Lease(tenant)),
        )
        .unwrap();
        let db = manager.open(tenant).await.unwrap();
        Self { db, root }
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
            realtime::OutboxService,
            repositories::TenantDeviceRepository,
            services::{
                BalanceService, ConfigService, DeviceService, EventService, NotificationService,
                PlanService, PricingPolicyService,
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
        // A closed pool makes any remaining accidental shared-store read fail immediately.
        let pg = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        pg.close().await;
        let cache = Arc::new(MemoryCache::default());
        let settings = Arc::new(ConfigService::new(pg.clone(), cache.clone(), "UTC".into()));
        let plan = PlanService::new(settings.clone()).create_tenant(tenant.db.clone(), vec![venue],
            serde_json::from_value(serde_json::json!({"name":"Two hours","price":100,"planType":"time_based","validityDays":30,"timeCredits":120,"deviceType":"PC","deviceSubType":"HIGH_END_PCS"})).unwrap(),None).await.unwrap().id;
        let balances = Arc::new(BalanceService::new(pg.clone(), cache.clone()));
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
        let outbox = OutboxService::new(pg.clone());
        let notifications = NotificationService::new(pg.clone(), outbox.clone(), cache.clone());
        let devices = DeviceService::new(
            pg.clone(),
            events.clone(),
            outbox,
            notifications,
            cache.clone(),
        );
        let sessions = gaming_cafe_api::services::SessionService::new(
            devices,
            balances.clone(),
            events,
            settings,
            PricingPolicyService::new(pg),
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

use axum::{
    middleware,
    routing::{delete, get, patch, post, put},
    Router,
};
use sqlx::PgPool;
use std::sync::Arc;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::{
    compression::CompressionLayer, cors::CorsLayer, limit::RequestBodyLimitLayer, trace::TraceLayer,
};
use uuid::Uuid;

use crate::cache::{create_cache, spawn_invalidation_listener, CacheService};
use crate::config::{create_pool_for, load_dotenv, Settings};
use crate::handlers;
use crate::middleware::{auth_middleware, global_rate_limit, request_context, request_deadline};
use crate::openapi::ApiDoc;
use crate::realtime::{Dispatcher, RealtimeHub};
use crate::services::{
    AuthService, BalanceService, ConfigService, CreditService, DeviceService, EventService,
    GameService, KioskOrderService, PlanService, PlayerPlanService, PricingPolicyService,
    ProductRecipeService, ProductService, SessionService, StaffGamingAllowanceService,
    StorageConfig, StorageService, TransactionService, UnitService, UserService,
};
use crate::sse::Broadcaster;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub struct AppState {
    pub control_db: Option<PgPool>,
    pub leases: Option<Arc<crate::control::LeaseClient>>,
    pub tenant_dbs: Option<Arc<crate::tenancy::TenantDbManager>>,
    pub tenant_provisioner: Option<Arc<crate::tenancy::TenantProvisioner>>,
    pub routing: Option<Arc<crate::routing::TenantRouter>>,
    pub cache: Arc<dyn CacheService>,
    pub settings: Arc<Settings>,
    pub metrics: Arc<crate::metrics::Metrics>,
    pub auth: AuthService,
    pub config: Arc<ConfigService>,
    pub users: Arc<UserService>,
    pub devices: DeviceService,
    pub plans: PlanService,
    pub pricing_rules: PricingPolicyService,
    pub player_plans: Arc<PlayerPlanService>,
    pub balances: Arc<BalanceService>,
    pub units: UnitService,
    pub sessions: SessionService,
    pub transactions: TransactionService,
    pub products: ProductService,
    pub product_recipes: ProductRecipeService,
    pub games: GameService,
    pub storage: StorageService,
    #[cfg(feature = "duckdb-analytics")]
    pub analytics: Arc<crate::analytics::registry::AnalyticsRegistry>,
    pub credit: Arc<CreditService>,
    pub staff_gaming_allowances: StaffGamingAllowanceService,
    pub kiosk_orders: KioskOrderService,
    pub ws_connections: Arc<crate::realtime::registry::ConnectionRegistry>,
}

impl AppState {
    pub async fn report_reader(
        &self,
        db: Arc<crate::tenancy::TenantDb>,
        locations: Option<Vec<Uuid>>,
    ) -> Result<crate::analytics::report_reader::ReportReader, crate::error::AppError> {
        #[cfg(feature = "duckdb-analytics")]
        {
            let analytics = self.analytics.get(db).await?;
            Ok(crate::analytics::report_reader::ReportReader::new(analytics).await?.scoped(locations))
        }
        #[cfg(not(feature = "duckdb-analytics"))]
        {
            let _ = (db, locations);
            Err(crate::analytics::report_reader::unavailable("native analytics is disabled"))
        }
    }
    /// Business handlers must use the authenticated tenant's locally owned database.
    /// Missing cell configuration is an error, never a PostgreSQL fallback.
    pub async fn business_db(
        &self,
        claims: &crate::dto::JwtUserClaims,
    ) -> Result<Arc<crate::tenancy::TenantDb>, crate::error::AppError> {
        let tenant = Uuid::parse_str(&claims.tenantId)
            .map_err(|_| crate::error::AppError::Unauthorized("Invalid tenant identity".into()))?;
        self.tenant_db(tenant).await?.ok_or_else(||
            crate::error::AppError::Api { code: "TENANT_STORAGE_UNAVAILABLE".into(), status: axum::http::StatusCode::SERVICE_UNAVAILABLE, details: None })
    }

    pub async fn tenant_db(
        &self,
        tenant_id: Uuid,
    ) -> Result<Option<Arc<crate::tenancy::TenantDb>>, crate::error::AppError> {
        let Some(manager) = &self.tenant_dbs else {
            return Ok(None);
        };
        // Request handling must never acquire ownership. A forbidden open means this
        // cell is not the current writer (or has fenced itself); only provisioning
        // and ownership orchestration may acquire a lease.
        let db = manager.open(tenant_id).await?;
        // A tenant may have been idle when its scheduled policy became due.
        crate::repositories::TenantPricingPolicyRepository::new(db.clone()).activate_due().await?;
        Ok(Some(db))
    }
}

pub async fn build_state() -> Arc<AppState> {
    load_dotenv();
    build_state_with_settings(Arc::new(Settings::from_env())).await
}

/// Construct one process using explicit configuration; operational storage is tenant SQLite.
pub async fn build_state_with_settings(settings: Arc<Settings>) -> Arc<AppState> {
    let realtime_hub = RealtimeHub::new(1024);
    let metrics = Arc::new(crate::metrics::Metrics::default());
    let background_jobs = crate::background::BackgroundJobs::new(crate::background::Limits::default()).expect("invalid background scheduler limits");
    metrics.attach_background_jobs(background_jobs.stats());
    let control_db = if let Some(url) = settings.control_database_url.as_deref() {
        let control_pool = create_pool_for(url, settings.as_ref()).await;
        crate::control::migrate(&control_pool)
            .await
            .expect("control-plane migrations failed");
        Some(control_pool)
    } else {
        None
    };
    let leases = if settings.roles.cell {
        settings
            .cell_id
            .zip(control_db.clone())
            .map(|(cell_id, pool)| {
                Arc::new(
                    crate::control::LeaseClient::new(
                        pool,
                        cell_id,
                        crate::control::LeaseConfig::default(),
                    )
                    .expect("invalid ownership lease configuration"),
                )
            })
    } else {
        None
    };
    if let Some(client) = &leases {
        client.clone().spawn_renewal();
    }
    let tenant_dbs = leases.as_ref().map(|leases| {
        let manager = Arc::new(
            crate::tenancy::TenantDbManager::new(
                crate::tenancy::TenantDbConfig {
                    root: settings.tenant_data_dir.clone(),
                    ..crate::tenancy::TenantDbConfig::default()
                },
                leases.clone(),
            )
            .expect("invalid tenant database configuration")
            .with_commit_notifier(Arc::new(realtime_hub.clone()))
            .with_background_jobs(background_jobs.clone()),
        );
        manager.clone().spawn_reaper();
        manager
    });
    #[cfg(feature = "duckdb-analytics")]
    let analytics = Arc::new(crate::analytics::registry::AnalyticsRegistry::default());
    let mut recoverer=None;
    let mut move_agent=None;
    let mut cold_agent=None;
    if tenant_dbs.is_some() && std::env::var("DISK_PRESSURE_MONITOR").as_deref()!=Ok("false") {
        std::fs::create_dir_all(&settings.tenant_data_dir).expect("tenant disk monitor directory");
        crate::disk::spawn(settings.tenant_data_dir.clone(),background_jobs.clone(),metrics.clone());
    }
    if let Some(manager) = tenant_dbs.clone() {
        crate::replication::worker::spawn_capture(manager.clone(), metrics.clone(), settings.tenant_data_dir.clone());
        match crate::replication::configured_worker(control_db.clone(), settings.cell_id, metrics.clone()) {
            Ok(Some(worker)) => {
                let recovery_analytics:Option<Arc<dyn crate::replication::recovery::RecoveryAnalytics>>={
                    #[cfg(feature="duckdb-analytics")]
                    {settings.nats_url.as_ref().map(|url|Arc::new(crate::replication::recovery::NativeAnalytics{registry:analytics.clone(),broker_url:url.clone()}) as Arc<dyn crate::replication::recovery::RecoveryAnalytics>)}
                    #[cfg(not(feature="duckdb-analytics"))]
                    {None}
                };
                recoverer=Some(Arc::new(crate::replication::recovery::Recoverer{
                    ledger:Arc::new(crate::replication::ledger::PostgresLedger{pool:control_db.clone().expect("replication control plane"),cell_id:settings.cell_id.expect("replication cell")}),
                    leases:leases.clone().expect("replication lease client"),databases:manager.clone(),store:worker.store.clone(),keys:worker.keys.clone(),staging_root:settings.tenant_data_dir.join("recovery-staging"),analytics:recovery_analytics,
                }));
                let worker=Arc::new(worker);
                move_agent=Some(Arc::new(crate::moving::agent::Agent{recovery:recoverer.as_ref().expect("move recovery context").clone(),worker:worker.clone()}));
                cold_agent=Some(Arc::new(crate::cold::Agent{recovery:recoverer.as_ref().expect("cold recovery context").clone(),worker:worker.clone()}));
                crate::replication::worker::spawn_upload(manager,worker);
            },
            Ok(None) => tracing::warn!("Remote replication disabled: configure REPLICATION_BUCKET and REPLICATION_KEY_DIR before production onboarding"),
            Err(error) => panic!("Invalid replication configuration: {error}"),
        }
    }
    if let Some(recoverer)=&recoverer {
        match recoverer.clone().resume_assigned().await {
            Ok(outcomes)=>for outcome in outcomes {tracing::info!(?outcome,"Startup tenant recovery outcome");},
            Err(error)=>tracing::warn!(%error,"Startup remote recovery unavailable"),
        }
        recoverer.clone().spawn_resume();
        Arc::new(crate::replication::drill::Drill{recovery:recoverer.clone(),metrics:metrics.clone()}).spawn();
    }
    if let Some(agent)=&move_agent {
        agent.resume_sources().await.expect("planned move source leases");
        agent.resume_completed().await.expect("planned move completion reconciliation");
        agent.tick().await.expect("planned move startup queue");
        agent.clone().spawn();
    }
    if let (Some(control), Some(client), Some(manager)) = (control_db.as_ref(), leases.as_ref(), tenant_dbs.as_ref()) {
        let recovered = crate::control::bootstrap::recover_assigned(control, client, manager, &settings.tenant_data_dir).await.expect("assigned tenant recovery failed");
        tracing::info!(recovered, "Recovered assigned tenant databases");
    }
    if let Some(agent)=&cold_agent {
        agent.heartbeat().await.expect("cold hydration readiness");
        agent.clone().spawn();
    }
    #[cfg(feature="duckdb-analytics")]
    if let (Some(recovery),Some(url))=(&recoverer,settings.nats_url.clone()) {
        crate::historical::hot::spawn(recovery.databases.clone(),Arc::new(crate::historical::hot::Writer{metrics:metrics.clone(),ledger:recovery.ledger.clone(),store:recovery.store.clone(),keys:recovery.keys.clone(),staging_root:settings.tenant_data_dir.join("hot-staging")}),url);
    }
    #[cfg(feature="duckdb-analytics")]
    if let Some(recovery)=&recoverer {
        Arc::new(crate::historical::archive::Worker{ledger:recovery.ledger.clone(),store:recovery.store.clone(),keys:recovery.keys.clone(),metrics:metrics.clone()}).spawn(recovery.databases.clone());
    }
    if let Some(recovery)=&recoverer {
        let state=Arc::new(crate::tenancy::rollout::State{pool:recovery.ledger.pool.clone()});
        let hooks=vec![Arc::new(crate::replication::snapshot::SnapshotHook{databases:recovery.databases.clone(),ledger:recovery.ledger.clone(),store:recovery.store.clone(),keys:Arc::new(recovery.keys.clone())}) as Arc<dyn crate::tenancy::MigrationHook>];
        let migrations=Arc::new(crate::tenancy::MigrationOrchestrator::new(recovery.ledger.cell_id,recovery.databases.clone(),state,hooks,Default::default()).expect("staged migration configuration"));
        crate::tenancy::rollout::spawn(recovery.ledger.pool.clone(),migrations);
    }
    if let (Some(control), Some(manager)) = (control_db.clone(), tenant_dbs.clone()) {
        crate::control::timezone::spawn(control.clone(), manager.clone());
        crate::control::staff_projection::spawn(control, manager);
    }
    let tenant_provisioner = match (control_db.clone(), leases.clone()) {
        (Some(control_pool), Some(leases)) => {
            let control = Arc::new(crate::tenancy::PostgresProvisioningControl::new(
                crate::control::Repository::new(control_pool),
                leases,
            ));
            Some(Arc::new(crate::tenancy::TenantProvisioner::new(
                settings.tenant_data_dir.clone(),
                control,
            )))
        }
        _ => None,
    };
    let routing = if settings.roles.router {
        match (control_db.clone(), settings.control_database_url.clone()) {
            (Some(control_pool), Some(control_url)) => {
                let cache = Arc::new(crate::routing::RoutingCache::new(control_pool.clone()));
                cache
                    .refresh_all()
                    .await
                    .expect("initial routing cache refresh failed");
                cache
                    .clone()
                    .spawn_refresh(std::time::Duration::from_secs(30));
                cache.clone().spawn_invalidation_listener(control_url);
                Some(Arc::new(
                    crate::routing::TenantRouter::new(cache, settings.cell_id)
                        .expect("routing proxy initialization failed")
                        .with_cold(crate::cold::Coordinator{pool:control_pool,wait_limit:std::time::Duration::from_secs(60)}),
                ))
            }
            _ => None,
        }
    } else {
        None
    };
    let cache = create_cache(settings.redis_url.as_deref()).await;
    if let (Some(manager), Some(url)) = (tenant_dbs.clone(), settings.nats_url.clone()) {
        #[cfg(feature = "duckdb-analytics")]
        crate::analytics::consumer::spawn(manager.clone(), metrics.clone(), url.clone(), analytics.clone());
        crate::analytics::publisher::spawn(manager, metrics.clone(), url);
    }
    spawn_invalidation_listener(cache.clone(), settings.redis_url.clone());
    let broadcaster = Broadcaster::new(100);
    let events = EventService::new(broadcaster);

    let config_service = Arc::new(ConfigService::new(cache.clone(), settings.cafe_timezone.clone()));
    let pricing_rules = PricingPolicyService::new();
    let ws_connections = Arc::new(crate::realtime::registry::ConnectionRegistry::default());

    let devices = DeviceService::new(events.clone(), cache.clone());
    let player_plans = Arc::new(PlayerPlanService::new());
    let balances = Arc::new(BalanceService::new(cache.clone()));

    let credit = Arc::new(CreditService::new());

    // Spawn the realtime dispatcher
    let dispatcher = Dispatcher::new(
        ws_connections.clone(),
        realtime_hub,
        tenant_dbs.clone(),
        metrics.clone(),
    );
    tokio::spawn(dispatcher.run());

    if let Some(manager) = tenant_dbs.clone() {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            interval.tick().await;
            loop {
                interval.tick().await;
                for db in manager.open_handles().await {
                    if let Err(error) = crate::repositories::TenantPricingPolicyRepository::new(db)
                        .activate_due()
                        .await
                    {
                        tracing::warn!(%error, "Tenant scheduled pricing activation failed");
                    }
                }
            }
        });
    }

    if let Some(manager) = tenant_dbs.clone() {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
                for db in manager.open_handles().await {
                    match crate::repositories::TenantNotificationRepository::new(db).cleanup_notifications(7).await {
                        Ok(count) if count > 0 => tracing::info!(count, "Tenant notification retention cleanup"),
                        Err(error) => tracing::warn!(%error, "Tenant notification retention cleanup failed"),
                        _ => {}
                    }
                }
            }
        });
    }

    let users = Arc::new(UserService::new());

    Arc::new(AppState {
        auth: AuthService::new(settings.clone()).with_control_pool(control_db.clone()),
        config: config_service.clone(),
        users: users.clone(),
        devices: devices.clone(),
        plans: PlanService::new(config_service.clone()),
        pricing_rules,
        player_plans: player_plans.clone(),
        balances: balances.clone(),
        units: UnitService::new(),
        sessions: SessionService::new(
            devices,
            balances.clone(),
            events.clone(),
            config_service.clone(),
            PricingPolicyService::new(),
            cache.clone(),
        ),
        transactions: TransactionService::new(),
        credit,
        staff_gaming_allowances: StaffGamingAllowanceService::new(
            balances.clone(), config_service.clone(),
        ),
        products: ProductService::new(),
        product_recipes: ProductRecipeService::new(),
        games: GameService::new(),
        storage: StorageService::new(StorageConfig::from_env()),
        #[cfg(feature = "duckdb-analytics")]
        analytics,
        kiosk_orders: KioskOrderService::new(config_service.clone()),
        ws_connections,
        control_db,
        leases,
        tenant_dbs,
        tenant_provisioner,
        routing,
        cache,
        settings,
        metrics,
    })
}

pub fn build_router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/", get(|| async { "Game Zone API" }))
        .route("/health", get(handlers::health::health_check_legacy))
        .route("/health/live", get(handlers::health::live_check))
        .route("/health/ready", get(handlers::health::ready_check))
        .route("/auth/login/admin", post(handlers::auth::login_admin))
        .route("/auth/login/staff", post(handlers::auth::login_staff))
        .route("/auth/me", get(handlers::auth::current_panel_user))
        .route("/auth/refresh", post(handlers::auth::refresh_panel_session))
        .route("/auth/staff-shift", post(handlers::auth::resume_staff_shift))
        .route(
            "/auth/admin-shift-close",
            post(handlers::auth::complete_admin_login),
        )
        .route("/access", get(handlers::access::snapshot))
        .route("/access/self", get(handlers::access::self_access))
        .route("/access/roles", post(handlers::access::create_role))
        .route(
            "/access/roles/{id}/delete",
            post(handlers::access::delete_role),
        )
        .route(
            "/access/roles/{id}",
            put(handlers::access::update_role).delete(handlers::access::delete_role),
        )
        .route("/access/members", post(handlers::access::create_member))
        .route("/access/members/{id}", put(handlers::access::save_member))
        .route(
            "/access/modules/{module}",
            put(handlers::access::save_module),
        )
        .route("/auth/login/panel", post(handlers::auth::login_panel))
        .route(
            "/auth/login/panel/mfa",
            post(handlers::auth::verify_panel_mfa),
        )
        .route("/auth/login/player", post(handlers::auth::login_player))
        .route(
            "/auth/register/player",
            post(handlers::auth::register_player),
        )
        .route("/auth/register", post(handlers::auth::register))
        .route("/stats/dashboard", get(handlers::stats::dashboard_stats))
        .route("/stats/business", get(handlers::stats::business_stats))
        .route(
            "/stats/staff-dashboard",
            get(handlers::stats::staff_dashboard_stats),
        )
        .route(
            "/stats/revenue/by-payment-method",
            get(handlers::stats::revenue_by_payment_method),
        )
        .route("/stats/usage", get(handlers::stats::usage_stats))
        .route(
            "/stats/finance/reconciliation",
            get(handlers::stats::finance_reconciliation_stats),
        )
        .route(
            "/stats/finance/deposits",
            get(handlers::stats::finance_deposit_stats),
        )
        .route(
            "/stats/finance/variance",
            get(handlers::stats::finance_variance_stats),
        )
        .route("/users", get(handlers::users::list_users))
        .route("/users/me/avatar", put(handlers::users::update_own_avatar))
        .route(
            "/users/{id}",
            get(handlers::users::get_user).put(handlers::users::update_user),
        )
        .route(
            "/users/{id}/password",
            put(handlers::users::change_password),
        )
        .route(
            "/devices",
            get(handlers::devices::list_devices).post(handlers::devices::create_device),
        )
        .route(
            "/devices/{id}",
            get(handlers::devices::get_device)
                .patch(handlers::devices::update_device)
                .delete(handlers::devices::delete_device),
        )
        .route(
            "/devices/{id}/status",
            patch(handlers::devices::update_device_status),
        )
        .route(
            "/devices/provision",
            post(handlers::devices::provision_device),
        )
        .route(
            "/products/{id}/location-scope",
            get(handlers::catalog_scope::get_product).put(handlers::catalog_scope::save_product),
        )
        .route(
            "/plans/{id}/location-scope",
            get(handlers::catalog_scope::get_plan).put(handlers::catalog_scope::save_plan),
        )
        .route("/plans/active", get(handlers::plans::get_active_plans))
        .route(
            "/plans",
            get(handlers::plans::list_plans).post(handlers::plans::create_plan),
        )
        .route(
            "/plans/{id}",
            get(handlers::plans::get_plan)
                .patch(handlers::plans::update_plan)
                .delete(handlers::plans::delete_plan),
        )
        .route(
            "/player-plans",
            get(handlers::balances::list_balances).post(handlers::balances::purchase_balance),
        )
        .route(
            "/player-plans/best-plan",
            get(handlers::balances::get_best_balance),
        )
        .route(
            "/player-plans/my-active-plans",
            get(handlers::balances::list_my_active_balances),
        )
        .route(
            "/player-plans/{id}/validate",
            post(handlers::balances::validate_access),
        )
        .route("/player-plans/{id}", get(handlers::balances::get_balance))
        .route(
            "/units",
            get(handlers::units::list_units).post(handlers::units::create_unit),
        )
        .route(
            "/units/{id}",
            get(handlers::units::get_unit)
                .put(handlers::units::update_unit)
                .delete(handlers::units::delete_unit),
        )
        .route(
            "/cash-registers/active/expected-closing",
            get(handlers::cash_registers::get_active_expected_closing),
        )
        .route(
            "/cash-registers/open",
            post(handlers::cash_registers::open_cash_register),
        )
        .route(
            "/cash-registers",
            get(handlers::cash_registers::list_cash_registers),
        )
        .route(
            "/cash-registers/{id}/close",
            patch(handlers::cash_registers::close_cash_register),
        )
        .route(
            "/cash-registers/{id}/reconcile",
            patch(handlers::cash_registers::reconcile_cash_register),
        )
        .route(
            "/cash-registers/{id}/update-opening",
            patch(handlers::cash_registers::update_opening_balance),
        )
        .route(
            "/cash-registers/{id}/entries",
            post(handlers::cash_registers::add_entry),
        )
        .route(
            "/cash-registers/{id}",
            get(handlers::cash_registers::get_cash_register),
        )
        .route("/shifts/handover", post(handlers::shifts::handover_shift))
        .route("/shifts/close", post(handlers::shifts::close_shift))
        .route("/shifts/clock-in", post(handlers::shifts::clock_in))
        .route(
            "/shifts/start-context",
            get(handlers::shifts::start_context),
        )
        .route("/shifts/start", post(handlers::shifts::start_shift))
        .route("/shifts/clock-out", patch(handlers::shifts::clock_out))
        .route("/shifts/active", get(handlers::shifts::get_active_shift))
        .route("/shifts", get(handlers::shifts::list_shifts))
        .route("/shifts/{id}", get(handlers::shifts::get_shift))
        .route(
            "/shifts/{id}/force-close",
            patch(handlers::shifts::force_close_shift),
        )
        .route(
            "/sessions",
            get(handlers::sessions::list_sessions).post(handlers::sessions::create_session),
        )
        .route(
            "/sessions/active",
            get(handlers::sessions::list_active_sessions),
        )
        .route("/sessions/{id}", get(handlers::sessions::get_session))
        .route("/sessions/{id}/end", patch(handlers::sessions::end_session))
        .route("/kiosk/sessions", post(handlers::kiosk::start_session))
        .route(
            "/kiosk/sessions/current",
            get(handlers::kiosk::current_session),
        )
        .route(
            "/kiosk/sessions/{id}/heartbeat",
            patch(handlers::kiosk::heartbeat_session),
        )
        .route(
            "/kiosk/sessions/{id}/end",
            patch(handlers::kiosk::end_session),
        )
        .route(
            "/kiosk/products",
            get(handlers::kiosk_orders::list_products),
        )
        .route("/kiosk/orders", post(handlers::kiosk_orders::place_order))
        .route(
            "/kiosk/orders/current",
            get(handlers::kiosk_orders::current_order),
        )
        .route(
            "/kiosk-orders/kitchen/tickets",
            get(handlers::kitchen::list),
        )
        .route(
            "/kiosk-orders/kitchen/tickets/{id}",
            patch(handlers::kitchen::advance),
        )
        .route("/kiosk-orders/kitchen/menu", get(handlers::kitchen::menu))
        .route(
            "/kiosk-orders/kitchen/menu/{id}",
            put(handlers::kitchen::save_menu),
        )
        .route(
            "/stats/finance/report",
            get(handlers::finance_report::report),
        )
        .route("/kiosk-orders", get(handlers::kiosk_orders::list_orders))
        .route(
            "/kiosk-orders/{id}",
            get(handlers::kiosk_orders::get_order).patch(handlers::kiosk_orders::update_order),
        )
        .route(
            "/kiosk-orders/{id}/convert",
            post(handlers::kiosk_orders::convert_order),
        )
        .route(
            "/transactions",
            get(handlers::transactions::list_transactions)
                .post(handlers::transactions::create_transaction),
        )
        .route(
            "/transactions/{id}",
            get(handlers::transactions::get_transaction)
                .patch(handlers::transactions::update_transaction),
        )
        .route(
            "/products",
            get(handlers::products::list_products).post(handlers::products::create_product),
        )
        .route(
            "/products/current-prices",
            get(handlers::products::current_prices),
        )
        .route(
            "/products/{id}",
            get(handlers::products::get_product)
                .patch(handlers::products::update_product)
                .delete(handlers::products::delete_product),
        )
        .route(
            "/products/{id}/recipe",
            get(handlers::products::get_recipe).put(handlers::products::save_recipe),
        )
        .route(
            "/games",
            get(handlers::games::list_games).post(handlers::games::create_game),
        )
        .route(
            "/games/{id}",
            get(handlers::games::get_game)
                .patch(handlers::games::update_game)
                .delete(handlers::games::delete_game),
        )
        .route("/uploads/presign", post(handlers::uploads::presign_upload))
        .route(
            "/expense-categories",
            get(handlers::expense_categories::list_expense_categories)
                .post(handlers::expense_categories::create_expense_category),
        )
        .route(
            "/expense-categories/{id}",
            get(handlers::expense_categories::get_expense_category)
                .patch(handlers::expense_categories::update_expense_category)
                .delete(handlers::expense_categories::delete_expense_category),
        )
        .route(
            "/vendors",
            get(handlers::vendors::list_vendors).post(handlers::vendors::create_vendor),
        )
        .route(
            "/vendors/{id}",
            get(handlers::vendors::get_vendor)
                .patch(handlers::vendors::update_vendor)
                .delete(handlers::vendors::delete_vendor),
        )
        .route(
            "/cash-deposits",
            get(handlers::cash_deposits::list_deposits)
                .post(handlers::cash_deposits::initiate_deposit),
        )
        .route(
            "/cash-deposits/{id}",
            get(handlers::cash_deposits::get_deposit),
        )
        .route(
            "/cash-deposits/{id}/approve",
            patch(handlers::cash_deposits::approve_deposit),
        )
        .route(
            "/cash-deposits/{id}/reject",
            patch(handlers::cash_deposits::reject_deposit),
        )
        .route("/users/{id}/totp/setup", post(handlers::users::setup_totp))
        .route(
            "/users/{id}/totp/verify",
            post(handlers::users::verify_totp_setup),
        )
        .route("/users/{id}/totp", delete(handlers::users::disable_totp))
        .route(
            "/expenses/summary",
            get(handlers::expenses::expense_summary),
        )
        .route(
            "/expenses",
            get(handlers::expenses::list_expenses).post(handlers::expenses::create_expense),
        )
        .route(
            "/expenses/{id}",
            get(handlers::expenses::get_expense)
                .patch(handlers::expenses::update_expense)
                .delete(handlers::expenses::delete_expense),
        )
        .route(
            "/expenses/{id}/approve",
            patch(handlers::expenses::approve_expense),
        )
        .route(
            "/expenses/{id}/reject",
            patch(handlers::expenses::reject_expense),
        )
        .route(
            "/inventory/locations",
            get(handlers::inventory::list_locations).post(handlers::inventory::create_location),
        )
        .route(
            "/inventory/locations/{id}",
            patch(handlers::inventory::update_location),
        )
        .route("/inventory/stock", get(handlers::inventory::list_stock))
        .route("/inventory/overview", get(handlers::procurement::overview))
        .route(
            "/inventory/movements",
            get(handlers::procurement::list_movements),
        )
        .route(
            "/inventory/reorder-rules",
            get(handlers::procurement::list_reorder_rules)
                .post(handlers::procurement::upsert_reorder_rule),
        )
        .route(
            "/inventory/reorder-suggestions",
            get(handlers::procurement::reorder_suggestions),
        )
        .route(
            "/inventory/purchase-orders",
            get(handlers::procurement::list_orders).post(handlers::procurement::create_order),
        )
        .route(
            "/inventory/purchase-orders/{id}",
            get(handlers::procurement::get_order).patch(handlers::procurement::update_order),
        )
        .route(
            "/inventory/purchase-orders/{id}/submit",
            post(handlers::procurement::submit_order),
        )
        .route(
            "/inventory/purchase-orders/{id}/approve",
            post(handlers::procurement::approve_order),
        )
        .route(
            "/inventory/purchase-orders/{id}/reject",
            post(handlers::procurement::reject_order),
        )
        .route(
            "/inventory/purchase-orders/{id}/mark-ordered",
            post(handlers::procurement::mark_ordered),
        )
        .route(
            "/inventory/purchase-orders/{id}/cancel",
            post(handlers::procurement::cancel_order),
        )
        .route(
            "/inventory/purchase-orders/{id}/receipts",
            post(handlers::procurement::receive_order),
        )
        .route(
            "/inventory/adjustments",
            get(handlers::inventory::list_adjustments).post(handlers::inventory::create_adjustment),
        )
        .route(
            "/inventory/adjustments/{id}",
            get(handlers::inventory::get_adjustment),
        )
        .route(
            "/inventory/receipts",
            get(handlers::inventory::list_receipts).post(handlers::inventory::create_receipt),
        )
        .route(
            "/inventory/receipts/summary",
            get(handlers::inventory::receipt_summary),
        )
        .route(
            "/inventory/transfer-requests",
            get(handlers::inventory::list_transfer_requests)
                .post(handlers::inventory::create_transfer_request),
        )
        .route(
            "/inventory/transfer-requests/{id}",
            get(handlers::inventory::get_transfer_request),
        )
        .route(
            "/inventory/transfer-requests/{id}/approve",
            patch(handlers::inventory::approve_transfer_request),
        )
        .route(
            "/inventory/transfer-requests/{id}/reject",
            patch(handlers::inventory::reject_transfer_request),
        )
        .route(
            "/inventory/transfer-requests/{id}/fulfill",
            patch(handlers::inventory::fulfill_transfer_request),
        )
        .route(
            "/inventory/waste-events",
            get(handlers::inventory::list_waste_events)
                .post(handlers::inventory::create_waste_event),
        )
        .route(
            "/inventory/waste-events/{id}",
            get(handlers::inventory::get_waste_event),
        )
        .route(
            "/inventory/waste-events/{id}/approve",
            patch(handlers::inventory::approve_waste_event),
        )
        .route(
            "/inventory/waste-events/{id}/reject",
            patch(handlers::inventory::reject_waste_event),
        )
        .route(
            "/inventory/waste/summary",
            get(handlers::inventory::waste_summary),
        )
        .route(
            "/users/{id}/credit-limit",
            patch(handlers::credit::update_credit_limit),
        )
        .route(
            "/users/{id}/staff-gaming-allowance",
            get(handlers::staff_gaming_allowance::get_staff_gaming_allowance)
                .patch(handlers::staff_gaming_allowance::update_staff_gaming_allowance),
        )
        .route(
            "/credit/accounts",
            get(handlers::credit::list_credit_accounts),
        )
        .route("/credit/summary", get(handlers::credit::credit_summary))
        .route(
            "/credit/players/{id}",
            get(handlers::credit::get_player_credit),
        )
        .route(
            "/credit/settlements",
            get(handlers::credit::list_settlements).post(handlers::credit::create_settlement),
        )
        .route(
            "/credit/settlements/{id}",
            get(handlers::credit::get_settlement),
        )
        .route(
            "/notifications",
            get(handlers::notifications::list_notifications),
        )
        .route(
            "/notifications/unread-count",
            get(handlers::notifications::unread_count),
        )
        .route(
            "/notifications/read-all",
            post(handlers::notifications::mark_all_read),
        )
        .route(
            "/notifications/{id}/read",
            patch(handlers::notifications::mark_read),
        )
        .route(
            "/activity-log",
            get(handlers::notifications::list_activity_log),
        )
        .route("/realtime", get(crate::realtime::handler::ws_upgrade))
        .route("/metrics", get(crate::metrics::prometheus))
        .route(
            "/realtime/rooms",
            get(handlers::realtime_rooms::list_rooms).post(handlers::realtime_rooms::create_room),
        )
        .route(
            "/realtime/rooms/{id}/members",
            post(handlers::realtime_rooms::add_member),
        )
        .route(
            "/realtime/rooms/{id}/members/{user_id}",
            delete(handlers::realtime_rooms::remove_member),
        )
        .route("/branding", get(handlers::config::branding))
        .route("/config", get(handlers::config::list_config))
        .route(
            "/config/{key}",
            get(handlers::config::get_config).put(handlers::config::upsert_config),
        )
        .route(
            "/organizations/{org_id}/settings/catalog",
            get(handlers::config::settings_catalog),
        )
        .route(
            "/organizations/{org_id}/locations",
            get(handlers::config::venue_locations).post(handlers::config::create_venue_location),
        )
        .route(
            "/organizations/{org_id}/locations/{location_id}",
            put(handlers::config::update_venue_location),
        )
        .route(
            "/organizations/{org_id}/settings/effective",
            get(handlers::config::effective_settings),
        )
        .route(
            "/organizations/{org_id}/settings/history",
            get(handlers::config::setting_history),
        )
        .route(
            "/organizations/{org_id}/settings/overrides/{key}",
            put(handlers::config::upsert_setting_override)
                .delete(handlers::config::delete_setting_override),
        )
        .route(
            "/organizations/{org_id}/configuration-snapshot",
            get(handlers::config::configuration_snapshot),
        )
        .route(
            "/organizations/{org_id}/pricing-rule-sets",
            get(handlers::pricing_rules::list_rule_sets)
                .post(handlers::pricing_rules::create_rule_set),
        )
        .route(
            "/organizations/{org_id}/pricing-rule-sets/{set_id}/versions",
            get(handlers::pricing_rules::list_versions)
                .post(handlers::pricing_rules::create_version),
        )
        .route(
            "/organizations/{org_id}/pricing-rule-sets/{set_id}/versions/{version_id}/validate",
            post(handlers::pricing_rules::validate_version),
        )
        .route(
            "/organizations/{org_id}/pricing-rule-sets/{set_id}/versions/{version_id}/simulate",
            post(handlers::pricing_rules::simulate_version),
        )
        .route(
            "/organizations/{org_id}/pricing-rule-sets/{set_id}/versions/{version_id}/publish",
            post(handlers::pricing_rules::publish_version),
        )
        .route(
            "/organizations/{org_id}/pricing-rule-sets/{set_id}/versions/{version_id}/rollback",
            post(handlers::pricing_rules::rollback_version),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::auth::authorize_tenant_request,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::routing::route_tenant_request,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state.clone());

    let rpc = crate::rpc::router(api.clone(), state.metrics.clone());
    let mut router = if state.settings.legacy_rest_enabled {
        tracing::warn!("LEGACY_REST_ENABLED is active; business REST endpoints are public");
        Router::new().merge(api).merge(rpc)
    } else {
        Router::new()
            .route("/", get(|| async { "Game Zone API" }))
            .route("/health", get(handlers::health::health_check_legacy))
            .route("/health/live", get(handlers::health::live_check))
            .route("/health/ready", get(handlers::health::ready_check))
            .route("/realtime", get(crate::realtime::handler::ws_upgrade))
            .route("/metrics", get(crate::metrics::prometheus))
            .with_state(state.clone())
            .merge(rpc)
    };

    if state.settings.legacy_rest_enabled && !state.settings.is_production() {
        router = router
            .merge(SwaggerUi::new("/api/docs").url("/api/docs/openapi.json", ApiDoc::openapi()));
    }

    Router::new()
        .merge(router)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(RequestBodyLimitLayer::new(2 * 1024 * 1024))
        .layer(middleware::from_fn(request_deadline))
        .layer(ConcurrencyLimitLayer::new(
            state.settings.max_concurrent_requests,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            global_rate_limit,
        ))
        .layer(CorsLayer::permissive())
        .layer(middleware::from_fn(request_context))
}

pub async fn create_app() -> Router {
    let state = build_state().await;
    build_router(state)
}

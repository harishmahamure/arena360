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
use crate::config::{create_pool, create_pool_for, load_dotenv, Settings};
use crate::handlers;
use crate::middleware::{auth_middleware, global_rate_limit, request_context, request_deadline};
use crate::openapi::ApiDoc;
use crate::realtime::{Dispatcher, OutboxService, RealtimeHub, RoomService};
use crate::services::{
    AuthService, BalanceService, CashDepositService, CashRegisterService, ConfigService,
    CreditService, DeviceService, EventService, ExpenseCategoryService, ExpenseService,
    GameService, InventoryService, KioskOrderService, NotificationService, PlanService,
    PlayerPlanService, PricingPolicyService, ProcurementService, ProductRecipeService,
    ProductService, SessionService, ShiftService, StaffGamingAllowanceService, StatsService,
    StorageConfig, StorageService, TransactionService, UnitService, UserService, VendorService,
};
use crate::sse::Broadcaster;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub struct AppState {
    pub db: PgPool,
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
    pub shifts: ShiftService,
    pub cash_registers: Arc<CashRegisterService>,
    pub cash_deposits: CashDepositService,
    pub transactions: TransactionService,
    pub products: ProductService,
    pub product_recipes: ProductRecipeService,
    pub games: GameService,
    pub storage: StorageService,
    pub expense_categories: ExpenseCategoryService,
    pub vendors: VendorService,
    pub expenses: ExpenseService,
    pub inventory: InventoryService,
    pub procurement: ProcurementService,
    pub stats: StatsService,
    pub credit: Arc<CreditService>,
    pub staff_gaming_allowances: StaffGamingAllowanceService,
    pub events: EventService,
    pub notifications: NotificationService,
    pub kiosk_orders: KioskOrderService,
    pub outbox: OutboxService,
    pub rooms: RoomService,
    pub ws_connections: Arc<crate::realtime::registry::ConnectionRegistry>,
}

impl AppState {
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
        let locations: Vec<(
            Uuid,
            String,
            String,
            bool,
            chrono::DateTime<chrono::Utc>,
            chrono::DateTime<chrono::Utc>,
        )> = sqlx::query_as(
            r#"SELECT id,slug,name,"isActive","createdAt","updatedAt"
                   FROM venue_locations
                   WHERE "organizationId"=$1
                   ORDER BY id"#,
        )
        .bind(tenant_id)
        .fetch_all(&self.db)
        .await?;
        crate::tenancy::sync_venue_locations(
            db.clone(),
            locations
                .into_iter()
                .map(|(id, slug, name, is_active, created_at, updated_at)| {
                    crate::tenancy::ProjectedVenueLocation {
                        id,
                        slug,
                        name,
                        is_active,
                        created_at,
                        updated_at,
                    }
                })
                .collect(),
        )
        .await?;
        Ok(Some(db))
    }
}

pub async fn build_state() -> Arc<AppState> {
    load_dotenv();
    let settings = Arc::new(Settings::from_env());
    let pool = create_pool(settings.as_ref()).await;
    let realtime_hub = RealtimeHub::new(1024);
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
            .with_commit_notifier(Arc::new(realtime_hub.clone())),
        );
        manager.clone().spawn_reaper();
        manager
    });
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
                let cache = Arc::new(crate::routing::RoutingCache::new(control_pool));
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
                        .expect("routing proxy initialization failed"),
                ))
            }
            _ => None,
        }
    } else {
        None
    };
    let cache = create_cache(settings.redis_url.as_deref()).await;
    let metrics = Arc::new(crate::metrics::Metrics::default());
    spawn_invalidation_listener(cache.clone(), settings.redis_url.clone());
    let broadcaster = Broadcaster::new(100);
    let events = EventService::new(broadcaster);

    let outbox = OutboxService::with_hub(pool.clone(), realtime_hub.clone());
    let config_service = Arc::new(
        ConfigService::new(pool.clone(), cache.clone(), settings.cafe_timezone.clone())
            .with_outbox(outbox.clone()),
    );
    let pricing_rules = PricingPolicyService::new(pool.clone()).with_outbox(outbox.clone());
    let notifications = NotificationService::new(pool.clone(), outbox.clone(), cache.clone());
    let rooms = RoomService::new(pool.clone());
    let ws_connections = Arc::new(crate::realtime::registry::ConnectionRegistry::default());

    let devices = DeviceService::new(
        pool.clone(),
        events.clone(),
        outbox.clone(),
        notifications.clone(),
        cache.clone(),
    );
    let player_plans = Arc::new(PlayerPlanService::new(pool.clone()));
    let balances = Arc::new(BalanceService::new(pool.clone(), cache.clone()));
    let balances_for_auth = balances.clone();

    let cash_registers = Arc::new(
        CashRegisterService::new(pool.clone(), cache.clone())
            .with_notifications(notifications.clone()),
    );

    let credit = Arc::new(
        CreditService::new(pool.clone(), cache.clone()).with_notifications(notifications.clone()),
    );

    // Spawn the realtime dispatcher
    let dispatcher = Dispatcher::new(
        pool.clone(),
        ws_connections.clone(),
        realtime_hub,
        tenant_dbs.clone(),
        metrics.clone(),
    );
    tokio::spawn(dispatcher.run());

    let scheduled_pricing = pricing_rules.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            if let Err(error) = scheduled_pricing.activate_due().await {
                tracing::warn!(%error, "Scheduled pricing activation failed");
            }
        }
    });

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

    let notifications_for_cleanup = notifications.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            match notifications_for_cleanup.cleanup(7).await {
                Ok(count) if count > 0 => {
                    tracing::info!("Notification retention cleanup: removed {count} rows");
                }
                Err(e) => {
                    tracing::warn!("Notification retention cleanup failed: {e}");
                }
                _ => {}
            }
        }
    });

    let users = Arc::new(UserService::new(pool.clone(), cache.clone()));

    let mut shifts = ShiftService::new(pool.clone());
    shifts.set_cash_registers(cash_registers.clone());
    shifts.set_notifications(notifications.clone());

    Arc::new(AppState {
        auth: AuthService::new(
            pool.clone(),
            settings.clone(),
            balances_for_auth,
            users.clone(),
        )
        .with_control_pool(control_db.clone()),
        config: config_service.clone(),
        users: users.clone(),
        devices: devices.clone(),
        plans: PlanService::new(pool.clone(), cache.clone(), config_service.clone()),
        pricing_rules,
        player_plans: player_plans.clone(),
        balances: balances.clone(),
        units: UnitService::new(pool.clone(), cache.clone()),
        sessions: SessionService::new(
            pool.clone(),
            devices,
            balances.clone(),
            events.clone(),
            outbox.clone(),
            notifications.clone(),
            settings.cafe_timezone.clone(),
            cache.clone(),
        ),
        shifts,
        cash_registers: cash_registers.clone(),
        cash_deposits: CashDepositService::new(
            pool.clone(),
            outbox.clone(),
            notifications.clone(),
            cash_registers.clone(),
        ),
        transactions: TransactionService::new(
            pool.clone(),
            balances.clone(),
            credit.clone(),
            events.clone(),
            outbox.clone(),
            notifications.clone(),
            settings.cafe_timezone.clone(),
            cache.clone(),
            config_service.clone(),
        ),
        credit,
        staff_gaming_allowances: StaffGamingAllowanceService::new(
            pool.clone(),
            users.clone(),
            balances.clone(),
            cache.clone(),
            config_service.clone(),
        ),
        products: ProductService::new(pool.clone(), cache.clone()),
        product_recipes: ProductRecipeService::new(pool.clone()),
        games: GameService::new(pool.clone(), cache.clone()),
        storage: StorageService::new(StorageConfig::from_env()),
        expense_categories: ExpenseCategoryService::new(pool.clone(), cache.clone()),
        vendors: VendorService::new(pool.clone()),
        expenses: ExpenseService::new(
            pool.clone(),
            CashRegisterService::new(pool.clone(), cache.clone()),
            ShiftService::new(pool.clone()),
            outbox.clone(),
            notifications.clone(),
        ),
        inventory: InventoryService::new(
            pool.clone(),
            settings.cafe_timezone.clone(),
            outbox.clone(),
            notifications.clone(),
            cache.clone(),
        ),
        procurement: ProcurementService::new(pool.clone()),
        stats: StatsService::new(crate::analytics::ClickHouse::from_env(), cache.clone()),
        kiosk_orders: KioskOrderService::new(
            pool.clone(),
            notifications.clone(),
            outbox.clone(),
            settings.cafe_timezone.clone(),
            config_service.clone(),
        ),
        notifications,
        outbox,
        rooms,
        ws_connections,
        db: pool,
        control_db,
        leases,
        tenant_dbs,
        tenant_provisioner,
        routing,
        cache,
        settings,
        metrics,
        events,
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

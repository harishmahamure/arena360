use axum::{
    middleware,
    routing::{delete, get, patch, post, put},
    Router,
};
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};

use crate::config::{create_pool, load_dotenv, Settings};
use crate::handlers;
use crate::middleware::auth_middleware;
use crate::openapi::ApiDoc;
use crate::realtime::{Dispatcher, OutboxService, RoomService};
use crate::services::{
    AuthService, BalanceService, CashDepositService, CashRegisterService, ConfigService,
    CreditService, DeviceService, EventService, ExpenseCategoryService, ExpenseService,
    PlanService, PlayerPlanService, ProductService, SessionService, ShiftService, StatsService,
    TransactionService, UnitService, UserService, VendorService,
};
use crate::sse::Broadcaster;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub struct AppState {
    pub db: PgPool,
    pub settings: Arc<Settings>,
    pub auth: AuthService,
    pub config: ConfigService,
    pub users: UserService,
    pub devices: DeviceService,
    pub plans: PlanService,
    pub player_plans: Arc<PlayerPlanService>,
    pub balances: Arc<BalanceService>,
    pub units: UnitService,
    pub sessions: SessionService,
    pub shifts: ShiftService,
    pub cash_registers: Arc<CashRegisterService>,
    pub cash_deposits: CashDepositService,
    pub transactions: TransactionService,
    pub products: ProductService,
    pub expense_categories: ExpenseCategoryService,
    pub vendors: VendorService,
    pub expenses: ExpenseService,
    pub stats: StatsService,
    pub credit: Arc<CreditService>,
    pub events: EventService,
    pub outbox: OutboxService,
    pub rooms: RoomService,
    pub ws_connections: Arc<tokio::sync::RwLock<Vec<Arc<tokio::sync::RwLock<crate::realtime::connection::Connection>>>>>,
}

pub async fn build_state() -> Arc<AppState> {
    load_dotenv();
    let settings = Arc::new(Settings::from_env());
    let pool = create_pool(settings.as_ref()).await;
    let broadcaster = Broadcaster::new(100);
    let events = EventService::new(broadcaster);

    let outbox = OutboxService::new(pool.clone());
    let rooms = RoomService::new(pool.clone());
    let ws_connections: Arc<tokio::sync::RwLock<Vec<Arc<tokio::sync::RwLock<crate::realtime::connection::Connection>>>>> =
        Arc::new(tokio::sync::RwLock::new(Vec::new()));

    let devices = DeviceService::new(pool.clone(), events.clone(), outbox.clone());
    let player_plans = Arc::new(PlayerPlanService::new(pool.clone()));
    let balances = Arc::new(BalanceService::new(pool.clone()));
    let balances_for_auth = balances.clone();

    let credit = Arc::new(CreditService::new(pool.clone()));

    // Spawn the realtime dispatcher
    let dispatcher = Dispatcher::new(pool.clone(), ws_connections.clone());
    tokio::spawn(dispatcher.run());

    Arc::new(AppState {
        auth: AuthService::new(pool.clone(), settings.clone(), balances_for_auth),
        config: ConfigService::new(pool.clone()),
        users: UserService::new(pool.clone()),
        devices: devices.clone(),
        plans: PlanService::new(pool.clone()),
        player_plans: player_plans.clone(),
        balances: balances.clone(),
        units: UnitService::new(pool.clone()),
        sessions: SessionService::new(pool.clone(), devices, balances.clone(), events.clone(), outbox.clone()),
        shifts: {
            let cash_reg = Arc::new(CashRegisterService::new(pool.clone()));
            let mut s = ShiftService::new(pool.clone());
            s.set_cash_registers(cash_reg);
            s
        },
        cash_registers: Arc::new(CashRegisterService::new(pool.clone())),
        cash_deposits: CashDepositService::new(pool.clone(), outbox.clone()),
        transactions: TransactionService::new(
            pool.clone(),
            balances,
            credit.clone(),
            events.clone(),
            outbox.clone(),
        ),
        credit,
        products: ProductService::new(pool.clone()),
        expense_categories: ExpenseCategoryService::new(pool.clone()),
        vendors: VendorService::new(pool.clone()),
        expenses: ExpenseService::new(
            pool.clone(),
            CashRegisterService::new(pool.clone()),
            ShiftService::new(pool.clone()),
            outbox.clone(),
        ),
        stats: StatsService::new(pool.clone()),
        outbox,
        rooms,
        ws_connections,
        db: pool,
        settings,
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
        .route("/auth/login/player", post(handlers::auth::login_player))
        .route("/auth/register", post(handlers::auth::register))
        .route("/auth/verify-otp", post(handlers::auth::verify_otp))
        .route("/stats/dashboard", get(handlers::stats::dashboard_stats))
        .route(
            "/stats/staff-dashboard",
            get(handlers::stats::staff_dashboard_stats),
        )
        .route(
            "/stats/revenue/by-payment-method",
            get(handlers::stats::revenue_by_payment_method),
        )
        .route("/stats/usage", get(handlers::stats::usage_stats))
        .route("/users", get(handlers::users::list_users))
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
        .route("/devices/register", post(handlers::devices::register_device))
        .route(
            "/devices/{id}/registration-code",
            post(handlers::devices::issue_registration_code),
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
            get(handlers::balances::list_balances)
                .post(handlers::balances::purchase_balance),
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
        .route(
            "/player-plans/{id}",
            get(handlers::balances::get_balance),
        )
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
            "/kiosk/sessions/{id}/end",
            patch(handlers::kiosk::end_session),
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
            "/products/{id}",
            get(handlers::products::get_product)
                .patch(handlers::products::update_product)
                .delete(handlers::products::delete_product),
        )
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
            "/users/{id}/credit-limit",
            patch(handlers::credit::update_credit_limit),
        )
        .route("/credit/accounts", get(handlers::credit::list_credit_accounts))
        .route(
            "/credit/players/{id}",
            get(handlers::credit::get_player_credit),
        )
        .route(
            "/credit/settlements",
            post(handlers::credit::create_settlement),
        )
        .route("/realtime", get(crate::realtime::handler::ws_upgrade))
        .route(
            "/realtime/rooms",
            get(handlers::realtime_rooms::list_rooms)
                .post(handlers::realtime_rooms::create_room),
        )
        .route(
            "/realtime/rooms/{id}/members",
            post(handlers::realtime_rooms::add_member),
        )
        .route(
            "/realtime/rooms/{id}/members/{user_id}",
            delete(handlers::realtime_rooms::remove_member),
        )
        .route("/config", get(handlers::config::list_config))
        .route(
            "/config/{key}",
            get(handlers::config::get_config).put(handlers::config::upsert_config),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state.clone());

    let mut router = Router::new().merge(api);

    if !state.settings.is_production() {
        router = router
            .merge(SwaggerUi::new("/api/docs").url("/api/docs/openapi.json", ApiDoc::openapi()));
    }

    Router::new()
        .merge(router)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
}

pub async fn create_app() -> Router {
    let state = build_state().await;
    build_router(state)
}

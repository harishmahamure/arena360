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
use crate::services::{
    AuthService, DeviceGameService, DeviceService, EventService, FileService, GameService,
    PlanService, PlayerPlanService, ProductService, SessionService, StatsService,
    StorageService, TransactionService, UnitService, UserService,
};
use crate::openapi::ApiDoc;
use crate::sse::Broadcaster;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub struct AppState {
    pub db: PgPool,
    pub settings: Arc<Settings>,
    pub auth: AuthService,
    pub users: UserService,
    pub devices: DeviceService,
    pub games: GameService,
    pub plans: PlanService,
    pub player_plans: Arc<PlayerPlanService>,
    pub units: UnitService,
    pub device_games: DeviceGameService,
    pub sessions: SessionService,
    pub transactions: TransactionService,
    pub products: ProductService,
    pub stats: StatsService,
    pub events: EventService,
    pub storage: Option<StorageService>,
    pub files: FileService,
}

pub async fn build_state() -> Arc<AppState> {
    load_dotenv();
    let settings = Arc::new(Settings::from_env());
    let pool = create_pool(settings.as_ref()).await;
    let broadcaster = Broadcaster::new(100);
    let events = EventService::new(broadcaster);

    let devices = DeviceService::new(pool.clone(), events.clone());
    let storage = StorageService::try_new(settings.as_ref());
    let files = FileService::new(
        pool.clone(),
        storage.clone(),
        settings.upload_max_size_bytes,
    );
    let player_plans = Arc::new(PlayerPlanService::new(pool.clone()));

    Arc::new(AppState {
        auth: AuthService::new(pool.clone(), settings.clone()),
        users: UserService::new(pool.clone()),
        devices: devices.clone(),
        games: GameService::new(pool.clone()),
        plans: PlanService::new(pool.clone()),
        player_plans: player_plans.clone(),
        units: UnitService::new(pool.clone()),
        device_games: DeviceGameService::new(pool.clone()),
        sessions: SessionService::new(pool.clone(), devices, player_plans.clone(), events.clone()),
        transactions: TransactionService::new(pool.clone(), player_plans, events.clone()),
        products: ProductService::new(pool.clone()),
        stats: StatsService::new(pool.clone()),
        storage,
        files,
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
        .route("/auth/register", post(handlers::auth::register))
        .route("/auth/verify-otp", post(handlers::auth::verify_otp))
        .route("/stats/dashboard", get(handlers::stats::dashboard_stats))
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
        .route("/devices", get(handlers::devices::list_devices).post(handlers::devices::create_device))
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
        .route("/games", get(handlers::games::list_games).post(handlers::games::create_game))
        .route(
            "/games/{id}",
            get(handlers::games::get_game)
                .patch(handlers::games::update_game)
                .delete(handlers::games::delete_game),
        )
        .route("/plans/active", get(handlers::plans::get_active_plans))
        .route("/plans", get(handlers::plans::list_plans).post(handlers::plans::create_plan))
        .route(
            "/plans/{id}",
            get(handlers::plans::get_plan)
                .patch(handlers::plans::update_plan)
                .delete(handlers::plans::delete_plan),
        )
        .route(
            "/player-plans",
            get(handlers::player_plans::list_player_plans)
                .post(handlers::player_plans::assign_plan),
        )
        .route(
            "/player-plans/best-plan",
            get(handlers::player_plans::get_best_plan),
        )
        .route(
            "/player-plans/my-active-plans",
            get(handlers::player_plans::list_my_active_plans),
        )
        .route(
            "/player-plans/{id}/validate",
            post(handlers::player_plans::validate_access),
        )
        .route(
            "/player-plans/{id}",
            get(handlers::player_plans::get_player_plan),
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
            "/device-games",
            get(handlers::device_games::list_device_games)
                .post(handlers::device_games::create_device_game),
        )
        .route(
            "/device-games/device/{device_id}",
            get(handlers::device_games::list_device_games_by_device),
        )
        .route(
            "/device-games/game/{game_id}",
            get(handlers::device_games::list_device_games_by_game),
        )
        .route(
            "/device-games/{id}",
            delete(handlers::device_games::delete_device_game),
        )
        .route(
            "/sessions",
            get(handlers::sessions::list_sessions).post(handlers::sessions::create_session),
        )
        .route("/sessions/active", get(handlers::sessions::list_active_sessions))
        .route(
            "/sessions/{id}",
            get(handlers::sessions::get_session),
        )
        .route(
            "/sessions/{id}/end",
            patch(handlers::sessions::end_session),
        )
        .route(
            "/transactions",
            get(handlers::transactions::list_transactions)
                .post(handlers::transactions::create_transaction),
        )
        .route(
            "/transactions/{id}",
            get(handlers::transactions::get_transaction),
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
            "/storage/upload-url",
            post(handlers::storage::generate_upload_url),
        )
        .route(
            "/storage/download-url",
            post(handlers::storage::generate_download_url),
        )
        .route("/storage/list", get(handlers::storage::list_objects))
        .route("/files/stats", get(handlers::files::get_storage_stats))
        .route(
            "/files",
            get(handlers::files::list_files).post(handlers::files::create_file),
        )
        .route(
            "/files/{id}/download-url",
            get(handlers::files::get_file_download_url),
        )
        .route("/files/{id}/archive", put(handlers::files::archive_file))
        .route("/files/{id}/activate", put(handlers::files::activate_file))
        .route(
            "/files/{id}",
            get(handlers::files::get_file)
                .put(handlers::files::update_file)
                .delete(handlers::files::delete_file),
        )
        .route("/sse", get(handlers::sse::sse_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state.clone());

    let mut router = Router::new().merge(api);

    if !state.settings.is_production() {
        router = router.merge(
            SwaggerUi::new("/api/docs").url("/api/docs/openapi.json", ApiDoc::openapi()),
        );
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

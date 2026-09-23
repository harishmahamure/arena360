use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use tracing::info;

use super::Settings;

pub async fn create_pool(settings: &Settings) -> PgPool {
    let connect_options: PgConnectOptions = settings
        .database_url
        .parse::<PgConnectOptions>()
        .expect("Invalid DATABASE_URL")
        .statement_cache_capacity(0);

    let pool = PgPoolOptions::new()
        .min_connections(settings.database_min_connections)
        .max_connections(settings.database_max_connections)
        .acquire_timeout(std::time::Duration::from_secs(
            settings.database_acquire_timeout_seconds,
        ))
        .idle_timeout(std::time::Duration::from_secs(
            settings.database_idle_timeout_seconds,
        ))
        .max_lifetime(std::time::Duration::from_secs(
            settings.database_max_lifetime_seconds,
        ))
        .connect_with(connect_options)
        .await
        .expect("Failed to connect to Postgres");

    info!(
        min_connections = settings.database_min_connections,
        max_connections = settings.database_max_connections,
        acquire_timeout_seconds = settings.database_acquire_timeout_seconds,
        statement_cache_capacity = 0,
        "Connected to Postgres (PgBouncer transaction-mode safe)"
    );

    pool
}

pub async fn ping(pool: &PgPool) -> bool {
    sqlx::query("SELECT 1").execute(pool).await.is_ok()
}

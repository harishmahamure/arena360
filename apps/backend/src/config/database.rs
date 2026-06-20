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
        .max_connections(settings.database_max_connections)
        .connect_with(connect_options)
        .await
        .expect("Failed to connect to Postgres");

    info!(
        max_connections = settings.database_max_connections,
        statement_cache_capacity = 0,
        "Connected to Postgres (PgBouncer transaction-mode safe)"
    );

    pool
}

pub async fn ping(pool: &PgPool) -> bool {
    sqlx::query("SELECT 1").execute(pool).await.is_ok()
}

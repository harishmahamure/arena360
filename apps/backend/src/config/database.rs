use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tracing::info;

use super::Settings;

pub async fn create_pool(settings: &Settings) -> PgPool {
        println!("Connecting to {} with {}", settings.database_url, settings.database_max_connections);


    let pool = PgPoolOptions::new()
        .max_connections(settings.database_max_connections)
        .connect(&settings.database_url)
        .await
        .expect("Failed to connect to Postgres");

    info!(
        max_connections = settings.database_max_connections,
        "Connected to Postgres"
    );

    pool
}

pub async fn ping(pool: &PgPool) -> bool {
    sqlx::query("SELECT 1").execute(pool).await.is_ok()
}

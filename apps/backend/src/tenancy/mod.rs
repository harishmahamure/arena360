//! Tenant context inside a cell (ADR-0043): resolving the tenant for a request,
//! the routing cache, the ownership lease client, and the signed entitlement cache.

use sqlx::{migrate::Migrator, SqlitePool};

static TENANT_MIGRATOR: Migrator = sqlx::migrate!("migrations/tenant");

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    TENANT_MIGRATOR.run(pool).await
}

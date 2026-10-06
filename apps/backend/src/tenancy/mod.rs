//! Tenant context inside a cell (ADR-0043): resolving the tenant for a request,
//! the routing cache, the ownership lease client, and the signed entitlement cache.

mod db;
mod migration;
mod provisioning;
mod retry;

use sqlx::{migrate::Migrator, SqliteConnection, SqlitePool};

pub use db::{tenant_path, TenantDb, TenantDbConfig, TenantDbManager, TenantLease};
pub use migration::{
    MigrationContext, MigrationHook, MigrationOrchestrator, MigrationOrchestratorConfig,
    MigrationOutcome, MigrationState, PendingTenantMigration, PostgresMigrationState,
};
pub use provisioning::{
    InitialSettingOverride, PostgresProvisioningControl, ProvisionTenant, ProvisionedTenant,
    ProvisioningControl, TenantProvisioner,
};
pub use retry::{
    is_sqlite_busy, retry_foreground, BackgroundBackoff, SqliteBusyMetrics, SqliteRetryConfig,
};

static TENANT_MIGRATOR: Migrator = sqlx::migrate!("migrations/tenant");

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    TENANT_MIGRATOR.run(pool).await
}

async fn migrate_connection(
    connection: &mut SqliteConnection,
) -> Result<(), sqlx::migrate::MigrateError> {
    TENANT_MIGRATOR.run_direct(connection).await
}

pub fn target_schema_version() -> i64 {
    TENANT_MIGRATOR
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or(0)
}

//! Tenant context inside a cell (ADR-0043): resolving the tenant for a request,
//! the routing cache, the ownership lease client, and the signed entitlement cache.

pub mod analytics_snapshot;
mod db;
mod location_projection;
mod migration;
pub mod rollout;
mod outbox;
mod provisioning;
mod retry;
mod values;

use sqlx::{migrate::Migrator, SqliteConnection, SqlitePool};

pub use crate::time::{format_sqlite_timestamp, parse_sqlite_timestamp, SqliteTimestampError};
pub(crate) use db::spool_wal as spool_connection_wal;
pub use db::{
    tenant_path, TenantCommitNotifier, TenantDb, TenantDbConfig, TenantDbManager, TenantLease,
};
pub use location_projection::{
    sync_venue_locations, LocationProjectionResult, ProjectedVenueLocation,
};
pub use migration::{
    MigrationAdmission, MigrationContext, MigrationHook, MigrationOrchestrator, MigrationOrchestratorConfig,
    MigrationOutcome, MigrationState, PendingTenantMigration, PostgresMigrationState,
};
pub use outbox::{
    write_outbox_event, write_outbox_event_on_connection, NewOutboxEvent, WrittenOutboxEvent,
};
pub use provisioning::{
    InitialSettingOverride, PostgresProvisioningControl, ProvisionTenant, ProvisionedTenant,
    ProvisioningControl, TenantProvisioner,
};
pub use retry::{
    is_sqlite_busy, retry_foreground, BackgroundBackoff, SqliteBusyMetrics, SqliteRetryConfig,
};
pub use values::{decimal_to_scale4, scale4_to_decimal, MoneyConversionError};

static TENANT_MIGRATOR: Migrator = sqlx::migrate!("migrations/tenant");

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    TENANT_MIGRATOR.run(pool).await
}

pub(crate) async fn migrate_connection(
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

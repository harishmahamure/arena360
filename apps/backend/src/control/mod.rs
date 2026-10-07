//! Control plane (ADR-0043, data-platform §5–14): tenants, cells, ownership leases,
//! users and memberships, licensing, and backup manifests in the global PostgreSQL.
//! Business operations never depend on it synchronously (§51).

pub mod entitlement;
pub mod lease;
pub mod identity;
pub mod staff_projection;
mod repository;

use sqlx::{migrate::Migrator, PgPool};

pub use lease::{LeaseClient, LeaseConfig, LeaseGrant, LeaseRepository};
pub use repository::{CreateTenant, Repository, Tenant};

static MIGRATOR: Migrator = sqlx::migrate!("migrations/control");

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(pool).await
}

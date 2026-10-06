//! Control plane (ADR-0043, data-platform §5–14): tenants, cells, ownership leases,
//! users and memberships, licensing, and backup manifests in the global PostgreSQL.
//! Business operations never depend on it synchronously (§51).

use sqlx::{migrate::Migrator, PgPool};

static MIGRATOR: Migrator = sqlx::migrate!("migrations/control");

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(pool).await
}

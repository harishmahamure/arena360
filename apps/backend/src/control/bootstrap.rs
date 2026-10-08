//! Restore runtime ownership and local projections for existing assigned tenants.
use super::LeaseClient;
use crate::{
    error::AppError,
    tenancy::{tenant_path, TenantDbManager},
};
use sqlx::PgPool;
use std::{path::Path, sync::Arc};
use uuid::Uuid;

pub async fn recover_assigned(
    pool: &PgPool,
    leases: &LeaseClient,
    manager: &Arc<TenantDbManager>,
    root: &Path,
) -> Result<usize, AppError> {
    let tenants: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM tenants WHERE owner_cell=$1 AND state='ACTIVE' AND storage_engine='SQLITE' ORDER BY id")
        .bind(leases.cell_id()).fetch_all(pool).await?;
    let mut recovered=0;
    for tenant in &tenants {
        if manager.move_pending(*tenant) {continue;}
        let path = tenant_path(root, *tenant);
        let file = tokio::fs::metadata(&path).await.map_err(|e| {
            AppError::Internal(format!(
                "assigned tenant {tenant} database is unavailable: {e}"
            ))
        })?;
        if !file.is_file() || file.len() == 0 {
            return Err(AppError::Internal(format!(
                "assigned tenant {tenant} database is empty or invalid"
            )));
        }
        leases.acquire_assigned(*tenant).await?;
        let db = manager.open(*tenant).await?;
        super::staff_projection::sync_tenant(pool, db).await?;
        recovered+=1;
    }
    Ok(recovered)
}

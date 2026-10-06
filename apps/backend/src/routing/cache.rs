use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use sqlx::postgres::PgListener;
use sqlx::{FromRow, PgPool};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::error::AppError;

pub const ROUTING_CHANGED_CHANNEL: &str = "arena_routing_changed";

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingTarget {
    pub tenant_id: Uuid,
    pub owner_cell: Uuid,
    pub address: String,
    pub ownership_generation: i64,
    pub schema_version: i64,
    pub timezone: String,
}

#[derive(Clone)]
pub struct RoutingCache {
    pool: PgPool,
    entries: Arc<RwLock<HashMap<Uuid, RoutingTarget>>>,
    refresh_gate: Arc<Mutex<()>>,
}

impl RoutingCache {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            entries: Arc::new(RwLock::new(HashMap::new())),
            refresh_gate: Arc::new(Mutex::new(())),
        }
    }

    pub async fn refresh_all(&self) -> Result<(), AppError> {
        let _refresh = self.refresh_gate.lock().await;
        let entries = sqlx::query_as::<_, RoutingTarget>(
            r#"SELECT tenant.id AS tenant_id,
                      tenant.owner_cell,
                      cell.address,
                      tenant.ownership_generation,
                      tenant.schema_version,
                      tenant.timezone
               FROM tenants tenant
               JOIN cells cell ON cell.id = tenant.owner_cell
               WHERE tenant.state NOT IN ('DELETED', 'FAILED', 'COLD')
                 AND cell.state <> 'OFFLINE'"#,
        )
        .fetch_all(&self.pool)
        .await?;
        let entries = entries
            .into_iter()
            .map(|entry| (entry.tenant_id, entry))
            .collect();
        *self.entries.write().await = entries;
        Ok(())
    }

    pub async fn resolve(&self, tenant_id: Uuid) -> Result<Option<RoutingTarget>, AppError> {
        if let Some(entry) = self.entries.read().await.get(&tenant_id).cloned() {
            return Ok(Some(entry));
        }
        self.refresh_tenant(tenant_id).await
    }

    pub async fn refresh_tenant(&self, tenant_id: Uuid) -> Result<Option<RoutingTarget>, AppError> {
        let _refresh = self.refresh_gate.lock().await;
        let entry = sqlx::query_as::<_, RoutingTarget>(
            r#"SELECT tenant.id AS tenant_id,
                      tenant.owner_cell,
                      cell.address,
                      tenant.ownership_generation,
                      tenant.schema_version,
                      tenant.timezone
               FROM tenants tenant
               JOIN cells cell ON cell.id = tenant.owner_cell
               WHERE tenant.id = $1
                 AND tenant.state NOT IN ('DELETED', 'FAILED', 'COLD')
                 AND cell.state <> 'OFFLINE'"#,
        )
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;
        let mut entries = self.entries.write().await;
        if let Some(entry) = &entry {
            entries.insert(tenant_id, entry.clone());
        } else {
            entries.remove(&tenant_id);
        }
        Ok(entry)
    }

    pub async fn invalidate(&self, tenant_id: Uuid) {
        self.entries.write().await.remove(&tenant_id);
    }

    pub fn spawn_refresh(self: Arc<Self>, interval: Duration) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.tick().await;
            loop {
                ticker.tick().await;
                if let Err(error) = self.refresh_all().await {
                    tracing::warn!(%error, "Routing cache refresh failed");
                }
            }
        })
    }

    pub fn spawn_invalidation_listener(
        self: Arc<Self>,
        database_url: String,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                match listen_for_invalidations(&self, &database_url).await {
                    Ok(()) => tracing::warn!("Routing invalidation listener disconnected"),
                    Err(error) => {
                        tracing::warn!(%error, "Routing invalidation listener failed");
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
    }
}

async fn listen_for_invalidations(
    cache: &RoutingCache,
    database_url: &str,
) -> Result<(), sqlx::Error> {
    let mut listener = PgListener::connect(database_url).await?;
    listener.listen(ROUTING_CHANGED_CHANNEL).await?;
    loop {
        let notification = listener.recv().await?;
        match Uuid::parse_str(notification.payload()) {
            Ok(tenant_id) => {
                if let Err(error) = cache.refresh_tenant(tenant_id).await {
                    cache.invalidate(tenant_id).await;
                    tracing::warn!(%tenant_id, %error, "Routing target refresh failed");
                }
            }
            Err(error) => {
                tracing::warn!(payload = notification.payload(), %error, "Invalid routing notification");
            }
        }
    }
}

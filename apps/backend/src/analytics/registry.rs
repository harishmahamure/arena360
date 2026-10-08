//! One canonical DuckDB connection per locally owned tenant, shared by ingestion and reports.
use super::tenant_db::TenantAnalytics;
use crate::{error::AppError, tenancy::TenantDb};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use uuid::Uuid;

type Entry = tokio::sync::Mutex<Weak<TenantAnalytics>>;

#[derive(Default)]
pub struct AnalyticsRegistry {
    entries: Mutex<HashMap<Uuid, Arc<Entry>>>,
}

impl AnalyticsRegistry {
    fn entry(&self, tenant: Uuid) -> Result<Arc<Entry>, AppError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| AppError::Internal("Analytics registry lock poisoned".into()))?;
        // Keep in-flight open gates, but do not accumulate entries for idle tenants.
        entries.retain(|_, entry| {
            Arc::strong_count(entry) > 1
                || entry
                    .try_lock()
                    .map(|handle| handle.strong_count() > 0)
                    .unwrap_or(true)
        });
        Ok(entries
            .entry(tenant)
            .or_insert_with(|| Arc::new(Entry::new(Weak::new())))
            .clone())
    }

    pub async fn get(&self, db: Arc<TenantDb>) -> Result<Arc<TenantAnalytics>, AppError> {
        db.ensure_current_owner()?;
        let entry = self.entry(db.tenant_id())?;
        let mut cached = entry.lock().await;
        if let Some(handle) = cached.upgrade() {
            if Arc::ptr_eq(handle.owner(), &db) && !handle.is_closed() {
                db.ensure_current_owner()?;
                return Ok(handle);
            }
            // The SQLite manager replaced an idle or fenced handle. Close the old
            // native connection before opening the same canonical file again.
            handle.close().await?;
            *cached = Weak::new();
        }
        let handle = TenantAnalytics::open_for_ingestion(db).await?;
        *cached = Arc::downgrade(&handle);
        Ok(handle)
    }

    pub async fn retire(&self,tenant:Uuid,generation:i64)->Result<(),AppError>{
        let entry=self.entry(tenant)?;
        let mut cached=entry.lock().await;
        if let Some(handle)=cached.upgrade(){
            if handle.owner().ownership_generation()==generation {handle.close().await?;*cached=Weak::new();}
        }
        Ok(())
    }

    /// A failed worker must not invalidate a replacement worker's connection.
    pub async fn invalidate(&self, handle: &Arc<TenantAnalytics>) -> Result<(), AppError> {
        let entry = self.entry(handle.tenant_id())?;
        let mut cached = entry.lock().await;
        if cached
            .upgrade()
            .is_some_and(|current| Arc::ptr_eq(&current, handle))
        {
            handle.close().await?;
            *cached = Weak::new();
        }
        Ok(())
    }
}

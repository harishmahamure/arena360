//! Coordinate derived-month readers with verified archive retirement.
use super::{
    archive::{self, Worker},
    objects::{self, Object},
    raw,
};
use crate::{error::AppError, tenancy::TenantDb};
use chrono::{Datelike, NaiveDate};
use futures::TryStreamExt;
use object_store::ObjectStoreExt;
use sqlx::{Postgres, Transaction};
use std::sync::Arc;
use uuid::Uuid;
fn fail(error: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("Archive handoff: {error}"))
}
/// Readers hold a shared lock until all selected monthly objects are materialized locally.
/// Writers hold it through upload/publication. Retirement takes the exclusive lock.
pub async fn lock_month(
    tx: &mut Transaction<'_, Postgres>,
    tenant: Uuid,
    month: NaiveDate,
    shared: bool,
) -> Result<(), AppError> {
    let function = if shared {
        "pg_advisory_xact_lock_shared"
    } else {
        "pg_advisory_xact_lock"
    };
    sqlx::query(&format!("SELECT {function}(hashtextextended($1,0))"))
        .bind(format!("historical-month:{tenant}:{month}"))
        .execute(&mut **tx)
        .await?;
    Ok(())
}
impl Worker {
    pub async fn handoff(&self, db: Arc<TenantDb>, id: Uuid) -> Result<(), AppError> {
        let started = std::time::Instant::now();
        let result = self.handoff_inner(db, id).await;
        self.metrics.historical_finished(
            "archive_handoff",
            result.is_ok(),
            started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        );
        result
    }
    async fn handoff_inner(&self, db: Arc<TenantDb>, id: Uuid) -> Result<(), AppError> {
        let job = archive::get(&self.ledger.pool, id).await?;
        if job.tenant_id != db.tenant_id()
            || !matches!(job.state.as_str(), "VERIFIED" | "PURGING" | "COMPLETE")
        {
            return Err(AppError::Conflict(
                "Hot retirement requires a verified archive for this tenant".into(),
            ));
        }
        if job.hot_cleaned_at.is_some() {
            return Ok(());
        }
        let mut tx = self.ledger.pool.begin().await?;
        // Acquire before a background slot: a writer holding a shared lock may need that slot.
        lock_month(&mut tx, job.tenant_id, job.period_start, false).await?;
        let _permit = match db.background_jobs() {
            Some(jobs) => Some(
                jobs.acquire(crate::background::Priority::ArchivePurge)
                    .await?,
            ),
            None => None,
        };
        let key = self.keys.read(job.tenant_id)?;
        let evidence: Vec<Object> = serde_json::from_value(job.objects.clone()).map_err(fail)?;
        let sum = raw::row_hash(&serde_json::to_string(&evidence).map_err(fail)?);
        if evidence.is_empty() || job.checksum_sha256.as_deref() != Some(&sum) {
            return Err(fail("Archive manifest checksum differs"));
        }
        let root = db
            .path()
            .parent()
            .unwrap()
            .join("historical/handoff")
            .join(id.to_string());
        std::fs::create_dir_all(&root).map_err(fail)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                .map_err(fail)?;
        }
        // Revalidate remote availability before removing the rebuildable copy.
        for object in &evidence {
            let path = root.join(format!("{}.parquet", object.table));
            objects::download(self.store.as_ref(), &key, object, &path).await?;
            std::fs::remove_file(path).map_err(fail)?;
        }
        self.ledger.lock_owner(&db, &mut tx).await?;
        let admitted: bool = sqlx::query_scalar("SELECT state IN ('VERIFIED','PURGING','COMPLETE') AND checksum_sha256=$2 FROM archive_manifests WHERE id=$1 FOR UPDATE")
            .bind(id).bind(&sum).fetch_one(&mut *tx).await?;
        if !admitted {
            return Err(AppError::Conflict(
                "Archive changed before hot retirement".into(),
            ));
        }
        sqlx::query("UPDATE hot_month_manifests SET state='RETIRED',retired_at=COALESCE(retired_at,clock_timestamp()) WHERE tenant_id=$1 AND period_start=$2")
            .bind(job.tenant_id).bind(job.period_start).execute(&mut *tx).await?;
        // Existing readers finished before retirement. New readers now choose the archive;
        // new writers check the verified archive and cannot republish this month.
        tx.commit().await?;
        let prefix = object_store::path::Path::from(format!(
            "tenants/{}/hot/{}/{:02}/",
            job.tenant_id,
            job.period_start.year(),
            job.period_start.month()
        ));
        let mut listed = self.store.list(Some(&prefix));
        let exact_prefix = format!("{prefix}/");
        while let Some(object) = listed.try_next().await.map_err(fail)? {
            if !object.location.as_ref().starts_with(&exact_prefix) {
                continue;
            }
            match self.store.delete(&object.location).await {
                Ok(()) | Err(object_store::Error::NotFound { .. }) => {}
                Err(error) => return Err(fail(error)),
            }
        }
        let mut tx = self.ledger.pool.begin().await?;
        self.ledger.lock_owner(&db, &mut tx).await?;
        sqlx::query("UPDATE archive_manifests SET hot_cleaned_at=clock_timestamp() WHERE id=$1 AND checksum_sha256=$2")
            .bind(id).bind(sum).execute(&mut *tx).await?;
        tx.commit().await?;
        std::fs::remove_dir_all(root).map_err(fail)?;
        Ok(())
    }
}

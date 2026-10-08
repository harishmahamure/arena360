//! Resumable cell-loss recovery. Operations activate before analytics rebuild.
use super::{
    crypto::TenantKeys,
    ledger::{GenerationReason, PostgresLedger},
    restore, snapshot,
};
use crate::{
    control::LeaseClient,
    error::AppError,
    tenancy::{TenantDb, TenantDbManager},
};
use async_trait::async_trait;
use object_store::ObjectStore;
use sqlx::Row;
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;
fn fail(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("Cell recovery: {e}"))
}
#[async_trait]
pub trait RecoveryAnalytics: Send + Sync {
    async fn rebuild(&self, db: Arc<TenantDb>) -> Result<(), AppError>;
}
pub struct Recoverer {
    pub ledger: Arc<PostgresLedger>,
    pub leases: Arc<LeaseClient>,
    pub databases: Arc<TenantDbManager>,
    pub store: Arc<dyn ObjectStore>,
    pub keys: TenantKeys,
    pub staging_root: PathBuf,
    pub analytics: Option<Arc<dyn RecoveryAnalytics>>,
}
#[derive(Debug, serde::Serialize)]
pub struct Outcome {
    pub tenant: Uuid,
    pub ownership_generation: i64,
    pub operations_ready: bool,
    pub analytics_ready: bool,
    pub error: Option<String>,
}
impl Recoverer {
    pub async fn affected(&self, cell: Uuid) -> Result<Vec<Uuid>, AppError> {
        Ok(sqlx::query_scalar("SELECT id FROM tenants WHERE owner_cell=$1 AND storage_engine='SQLITE' AND state NOT IN ('PROVISIONING','DELETED','FAILED','COLD') ORDER BY id").bind(cell).fetch_all(&self.ledger.pool).await?)
    }
    pub async fn resume_assigned(self: Arc<Self>) -> Result<Vec<Outcome>, AppError> {
        let rows=sqlx::query("SELECT t.id,t.state,COALESCE(j.phase='COMPLETE' AND j.analytics_ready_at IS NULL,false) AS analytics_pending FROM tenants t LEFT JOIN tenant_recovery_jobs j ON j.tenant_id=t.id AND j.ownership_generation=t.ownership_generation WHERE t.owner_cell=$1 AND t.storage_engine='SQLITE' AND t.state IN ('ACTIVE','RESTORING') ORDER BY t.id").bind(self.ledger.cell_id).fetch_all(&self.ledger.pool).await?;
        let mut outcomes = vec![];
        for row in rows {
            let tenant: Uuid = row.get(0);
            if row.get::<String, _>(1) == "ACTIVE"
                && self.databases.recovery_path(tenant).is_file()
                && !self.databases.recovery_pending(tenant)
                && !(self.analytics.is_some() && row.get::<bool, _>(2))
            {
                continue;
            }
            match self.clone().recover_tenant(tenant).await {
                Ok(o) => outcomes.push(o),
                Err(e) => outcomes.push(Outcome {
                    tenant,
                    ownership_generation: 0,
                    operations_ready: false,
                    analytics_ready: false,
                    error: Some(e.to_string()),
                }),
            }
        }
        Ok(outcomes)
    }
    pub fn spawn_resume(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            tick.tick().await;
            loop {
                tick.tick().await;
                match self.clone().resume_assigned().await {
                    Ok(outcomes) => {
                        for outcome in outcomes {
                            if outcome.error.is_some() {
                                tracing::warn!(
                                    ?outcome,
                                    "Assigned tenant recovery remains pending"
                                );
                            }
                        }
                    }
                    Err(error) => tracing::warn!(%error,"Assigned recovery registry unavailable"),
                }
            }
        });
    }
    pub async fn recover_cell(self: Arc<Self>, cell: Uuid) -> Result<Vec<Outcome>, AppError> {
        let tenants = self.affected(cell).await?;
        let mut outcomes = vec![];
        for tenant in tenants {
            match self.clone().recover_tenant(tenant).await {
                Ok(outcome) => outcomes.push(outcome),
                Err(error) => outcomes.push(Outcome {
                    tenant,
                    ownership_generation: 0,
                    operations_ready: false,
                    analytics_ready: false,
                    error: Some(error.to_string()),
                }),
            }
        }
        Ok(outcomes)
    }
    pub async fn recover_tenant(self: Arc<Self>, tenant: Uuid) -> Result<Outcome, AppError> {
        // One recoverer per tenant, including across processes. This transaction
        // locks only the job advisory key; lease renewal remains independent.
        let mut serial = self.ledger.pool.begin().await?;
        let acquired: bool =
            sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
                .bind(format!("recovery:{tenant}"))
                .fetch_one(&mut *serial)
                .await?;
        if !acquired {
            return Err(AppError::Conflict(
                "Tenant recovery is already running".into(),
            ));
        }
        let previous:Option<String>=sqlx::query_scalar("SELECT j.phase FROM tenant_recovery_jobs j JOIN tenants t ON t.id=j.tenant_id WHERE t.id=$1 AND t.owner_cell=$2 AND j.cell_id=$2 AND j.ownership_generation=t.ownership_generation").bind(tenant).bind(self.ledger.cell_id).fetch_optional(&self.ledger.pool).await?;
        if self.databases.recovery_path(tenant).exists()
            && !self.databases.recovery_pending(tenant)
            && previous.as_deref() != Some("COMPLETE")
        {
            return Err(AppError::Conflict(
                "An existing operational tenant image cannot be replaced by cell-loss recovery"
                    .into(),
            ));
        }
        let grant = self.leases.acquire_for_recovery(tenant).await?;
        let generation = grant.ownership_generation;
        let job=sqlx::query("INSERT INTO tenant_recovery_jobs(id,tenant_id,cell_id,ownership_generation,source_generation) SELECT $1,id,$2,$3,current_replication_generation FROM tenants WHERE id=$4 AND owner_cell=$2 AND ownership_generation=$3 AND current_replication_generation IS NOT NULL ON CONFLICT(tenant_id,ownership_generation) DO UPDATE SET last_error=NULL RETURNING id,phase,capture_number,source_generation").bind(Uuid::new_v4()).bind(self.ledger.cell_id).bind(generation).bind(tenant).fetch_optional(&self.ledger.pool).await?.ok_or_else(||fail("No selected backup generation exists for this tenant"))?;
        let id: Uuid = job.get(0);
        let phase: String = job.get(1);
        let capture: Option<i64> = job.get(2);
        let critical = match self.databases.background_jobs() {
            Some(j) => Some(
                j.acquire(crate::background::Priority::CriticalRecovery)
                    .await?,
            ),
            None => None,
        };
        let result=async {
            if phase=="COMPLETE" && !self.databases.recovery_pending(tenant) {
                self.activate(tenant,generation,id).await?;
                return self.databases.open(tenant).await;
            }
            let image=if phase=="DOWNLOADING" {
                let restored=restore::restore(tenant,self.leases.clone(),&self.ledger,self.store.as_ref(),&self.keys,&self.staging_root,None,restore::Limits::default()).await?;
                let expected:Uuid=job.get(3);
                if restored.generation!=expected {return Err(fail("Recovery source generation changed"));}
                Some(restored)
            } else {None};
            let number=image.as_ref().map(|r|r.capture_number).unwrap_or(capture.unwrap_or(0) as u64);
            let image_path=image.as_ref().map(|r|r.image.as_path());
            let recovered=image.as_ref().map(|r|r.recovered_at);
            let checksum=image.as_ref().map(|r|snapshot::hash_file(&r.image)).transpose()?;
            let this=self.clone();
            let db=self.databases.with_recovery_image(tenant,phase!="DOWNLOADING",image_path,number,id,move |db|Box::pin(async move {
                this.leases.ensure_writable(tenant,generation)?;
                let mut tx=this.ledger.pool.begin().await?;
                this.ledger.lock_owner(&db,&mut tx).await?;
                sqlx::query("UPDATE tenant_recovery_jobs SET phase=CASE WHEN phase='DOWNLOADING' THEN 'INSTALLED' ELSE phase END,capture_number=COALESCE(capture_number,$2),recovered_at=COALESCE(recovered_at,$3),image_checksum=COALESCE(image_checksum,$4) WHERE id=$1").bind(id).bind(number as i64).bind(recovered).bind(checksum).execute(&mut *tx).await?;
                tx.commit().await?;
                this.ledger.start_generation(&db,GenerationReason::Restore,id).await?;
                if phase!="BASELINED" {
                    snapshot::take(db.clone(),&this.ledger,this.store.as_ref(),&this.keys,snapshot::Kind::Baseline).await?;
                    let version=db.with_writer(|c|Box::pin(async move {
                        let has:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='_sqlx_migrations')").fetch_one(&mut *c).await?;
                        Ok(if has {sqlx::query_scalar::<_,i64>("SELECT COALESCE(MAX(version),0) FROM _sqlx_migrations WHERE success=1").fetch_one(c).await?} else {0})
                    })).await?;
                    let target=crate::tenancy::target_schema_version();
                    if version>target {return Err(fail("Restored schema is newer than this cell supports"));}
                    if version<target {
                        snapshot::take(db.clone(),&this.ledger,this.store.as_ref(),&this.keys,snapshot::Kind::PreMigration(target)).await?;
                        db.with_writer(|c|Box::pin(async move {crate::tenancy::migrate_connection(c).await.map_err(fail)?;Ok(())})).await?;
                    }
                    // Post verification also runs on a retry after migration commit.
                    snapshot::take(db.clone(),&this.ledger,this.store.as_ref(),&this.keys,snapshot::Kind::PostMigration(target)).await?;
                    let mut tx=this.ledger.pool.begin().await?;
                    this.ledger.lock_owner(&db,&mut tx).await?;
                    sqlx::query("UPDATE tenants SET schema_version=$2 WHERE id=$1").bind(tenant).bind(target).execute(&mut *tx).await?;
                    sqlx::query("UPDATE tenant_recovery_jobs SET phase='BASELINED' WHERE id=$1").bind(id).execute(&mut *tx).await?;
                    tx.commit().await?;
                }
                crate::control::staff_projection::sync_tenant(&this.ledger.pool,db.clone()).await?;
                let checks=db.with_writer(|c|Box::pin(async move {Ok(sqlx::query_scalar::<_,String>("PRAGMA integrity_check").fetch_all(c).await?)})).await?;
                if checks!=["ok"] {return Err(fail("Recovered operational database failed integrity check"));}
                this.activate(tenant,generation,id).await?;
                Ok(db)
            })).await;
            if let Some(image)=image {if let Some(dir)=image.image.parent() {let _=std::fs::remove_dir_all(dir);}}
            Ok::<_,AppError>(db?)
        }.await;
        drop(critical);
        let db = match result {
            Ok(db) => db,
            Err(error) => {
                let _ = sqlx::query("UPDATE tenant_recovery_jobs SET last_error=$2 WHERE id=$1")
                    .bind(id)
                    .bind(error.to_string())
                    .execute(&self.ledger.pool)
                    .await;
                return Err(error);
            }
        };
        // The manager has published the handle and routing is ACTIVE now.
        // Analytics failure keeps operations online and remains retryable.
        let mut analytics_ready = false;
        let mut error = None;
        if let Some(analytics) = &self.analytics {
            match tokio::time::timeout(std::time::Duration::from_secs(120),analytics.rebuild(db)).await.map_err(fail).and_then(|r|r) {
                Ok(()) => {
                    sqlx::query("UPDATE tenant_recovery_jobs SET analytics_ready_at=clock_timestamp(),last_error=NULL WHERE id=$1").bind(id).execute(&self.ledger.pool).await?;
                    analytics_ready = true;
                }
                Err(e) => {
                    error = Some(e.to_string());
                    sqlx::query("UPDATE tenant_recovery_jobs SET last_error=$2 WHERE id=$1")
                        .bind(id)
                        .bind(&error)
                        .execute(&self.ledger.pool)
                        .await?;
                }
            }
        }
        serial.commit().await?;
        Ok(Outcome {
            tenant,
            ownership_generation: generation,
            operations_ready: true,
            analytics_ready,
            error,
        })
    }
    async fn activate(&self, tenant: Uuid, generation: i64, job: Uuid) -> Result<(), AppError> {
        self.leases.ensure_writable(tenant, generation)?;
        let mut tx = self.ledger.pool.begin().await?;
        let locked:Option<bool>=sqlx::query_scalar("SELECT l.expires_at>clock_timestamp()+INTERVAL '30 seconds' FROM tenants t JOIN tenant_leases l ON l.tenant_id=t.id WHERE t.id=$1 AND t.owner_cell=$2 AND l.owner_cell=$2 AND t.ownership_generation=$3 AND l.ownership_generation=$3 FOR UPDATE OF t,l").bind(tenant).bind(self.ledger.cell_id).bind(generation).fetch_optional(&mut *tx).await?;
        if locked != Some(true) {
            return Err(fail(
                "Recovery ownership expired or changed before activation",
            ));
        }
        let ready:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenants t JOIN tenant_leases l ON l.tenant_id=t.id JOIN replication_generations g ON g.id=t.current_replication_generation AND g.transition_id=$4 AND g.ownership_generation=$3 JOIN snapshot_manifests s ON s.generation_id=t.current_replication_generation WHERE t.id=$1 AND t.owner_cell=$2 AND l.owner_cell=$2 AND t.ownership_generation=$3 AND l.ownership_generation=$3 AND l.expires_at>clock_timestamp()+INTERVAL '30 seconds' AND s.verified_at IS NOT NULL AND s.retired_at IS NULL AND s.source_checksum_sha256 IS NOT NULL)").bind(tenant).bind(self.ledger.cell_id).bind(generation).bind(job).fetch_one(&mut *tx).await?;
        if !ready {
            return Err(fail(
                "Verified baseline or ownership is unavailable before activation",
            ));
        }
        sqlx::query("UPDATE tenants SET state='ACTIVE',updated_at=clock_timestamp() WHERE id=$1 AND owner_cell=$2 AND ownership_generation=$3").bind(tenant).bind(self.ledger.cell_id).bind(generation).execute(&mut *tx).await?;
        sqlx::query("UPDATE tenant_recovery_jobs SET phase='COMPLETE',operations_ready_at=COALESCE(operations_ready_at,clock_timestamp()),last_error=NULL WHERE id=$1").bind(job).execute(&mut *tx).await?;
        sqlx::query("SELECT pg_notify($1,$2)")
            .bind(crate::routing::ROUTING_CHANGED_CHANNEL)
            .bind(tenant.to_string())
            .execute(&mut *tx)
            .await?;
        self.leases.ensure_writable(tenant, generation)?;
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(feature = "duckdb-analytics")]
pub struct NativeAnalytics {
    pub registry: Arc<crate::analytics::registry::AnalyticsRegistry>,
    pub broker_url: String,
}
#[cfg(feature = "duckdb-analytics")]
#[async_trait]
impl RecoveryAnalytics for NativeAnalytics {
    async fn rebuild(&self, db: Arc<TenantDb>) -> Result<(), AppError> {
        let client = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            async_nats::connect(&self.broker_url),
        )
        .await
        .map_err(fail)?
        .map_err(fail)?;
        let broker = async_nats::jetstream::new(client);
        let analytics = self.registry.get(db.clone()).await?;
        if let Ok(reader)=crate::analytics::report_reader::ReportReader::new(analytics.clone()).await {if reader.ensure_ready().await.is_ok(){return Ok(());}}
        crate::analytics::rebuild::rebuild(db, analytics, &broker).await
    }
}

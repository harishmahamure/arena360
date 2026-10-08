use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use futures::future::join_all;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, Copy)]
pub struct LeaseConfig {
    pub lease_duration: Duration,
    pub renewal_interval: Duration,
    pub fence_before_expiry: Duration,
    pub reassignment_skew: Duration,
}

impl LeaseConfig {
    pub fn validate(self) -> Result<Self, AppError> {
        if self.lease_duration.is_zero()
            || self.renewal_interval.is_zero()
            || self.fence_before_expiry >= self.lease_duration
            || self.renewal_interval >= self.lease_duration - self.fence_before_expiry
        {
            return Err(AppError::Internal(
                "invalid ownership lease timing configuration".into(),
            ));
        }
        Ok(self)
    }
}

impl Default for LeaseConfig {
    fn default() -> Self {
        Self {
            lease_duration: Duration::from_secs(5 * 60),
            renewal_interval: Duration::from_secs(60),
            fence_before_expiry: Duration::from_secs(30),
            reassignment_skew: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct LeaseGrant {
    pub tenant_id: Uuid,
    pub owner_cell: Uuid,
    pub ownership_generation: i64,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct MeasuredLease {
    grant: LeaseGrant,
    writable_until: Instant,
}

impl MeasuredLease {
    fn new(grant: LeaseGrant, request_sent_at: Instant, config: LeaseConfig) -> Self {
        Self {
            grant,
            writable_until: request_sent_at + config.lease_duration - config.fence_before_expiry,
        }
    }

    fn is_writable_at(&self, now: Instant) -> bool {
        now < self.writable_until
    }
}

#[derive(Clone)]
pub struct LeaseRepository {
    pool: PgPool,
}

impl LeaseRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn acquire(
        &self,
        tenant_id: Uuid,
        cell_id: Uuid,
        config: LeaseConfig,
    ) -> Result<LeaseGrant, AppError> {
        self.acquire_with_activation(tenant_id, cell_id, config, true, false, false)
            .await
    }

    pub async fn acquire_for_provisioning(
        &self,
        tenant_id: Uuid,
        cell_id: Uuid,
        config: LeaseConfig,
    ) -> Result<LeaseGrant, AppError> {
        self.acquire_with_activation(tenant_id, cell_id, config, false, false, false)
            .await
    }

    /// Restart recovery may only renew the currently assigned, active tenant.
    pub async fn acquire_assigned(
        &self,
        tenant_id: Uuid,
        cell_id: Uuid,
        config: LeaseConfig,
    ) -> Result<LeaseGrant, AppError> {
        self.acquire_with_activation(tenant_id, cell_id, config, false, true, false)
            .await
    }

    pub async fn acquire_for_recovery(&self, tenant_id: Uuid, cell_id: Uuid, config: LeaseConfig) -> Result<LeaseGrant,AppError> {
        self.acquire_with_activation(tenant_id,cell_id,config,false,false,true).await
    }

    async fn acquire_with_activation(
        &self,
        tenant_id: Uuid,
        cell_id: Uuid,
        config: LeaseConfig,
        activate: bool,
        require_assignment: bool,
        restoring: bool,
    ) -> Result<LeaseGrant, AppError> {
        let config = config.validate()?;
        let mut tx = self.pool.begin().await?;
        let cell_state: Option<String> =
            sqlx::query_scalar("SELECT state FROM cells WHERE id = $1 FOR SHARE")
                .bind(cell_id)
                .fetch_optional(&mut *tx)
                .await?;
        if !matches!(cell_state.as_deref(), Some("ACTIVE" | "DRAINING")) {
            return Err(AppError::Forbidden("Cell is not available".into()));
        }

        let tenant: Option<(Option<Uuid>, i64, String)> = sqlx::query_as(
            "SELECT owner_cell, ownership_generation, state FROM tenants WHERE id = $1 FOR UPDATE",
        )
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((current_owner, current_generation, tenant_state)) = tenant else {
            return Err(AppError::NotFound("Tenant not found".into()));
        };
        if require_assignment && (current_owner != Some(cell_id) || tenant_state != "ACTIVE") {
            return Err(AppError::Forbidden(
                "Tenant is not active on this cell".into(),
            ));
        }
        if matches!(tenant_state.as_str(), "DELETED" | "FAILED") {
            return Err(AppError::Forbidden(
                "Tenant cannot acquire an ownership lease".into(),
            ));
        }

        let existing: Option<(Uuid, i64, DateTime<Utc>)> = sqlx::query_as(
            "SELECT owner_cell, ownership_generation, expires_at \
             FROM tenant_leases WHERE tenant_id = $1 FOR UPDATE",
        )
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await?;
        let database_now: DateTime<Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
            .fetch_one(&mut *tx)
            .await?;
        let skew = chrono::Duration::from_std(config.reassignment_skew)
            .map_err(|_| AppError::Internal("reassignment skew is too large".into()))?;
        if let Some((lease_owner, lease_generation, _)) = existing {
            if current_owner != Some(lease_owner) || current_generation != lease_generation {
                return Err(AppError::Internal(
                    "tenant ownership and lease generation disagree".into(),
                ));
            }
        }
        let taking_ownership = !matches!(existing, Some((owner, _, _)) if owner == cell_id);
        if taking_ownership && cell_state.as_deref() != Some("ACTIVE") {
            return Err(AppError::Forbidden(
                "Draining cells cannot acquire new tenants".into(),
            ));
        }

        let generation = match existing {
            Some((owner, generation, _)) if owner == cell_id => generation,
            Some((_, _, expires_at)) if database_now < expires_at + skew => {
                return Err(AppError::Conflict(
                    "Tenant ownership lease is held by another cell".into(),
                ));
            }
            _ => current_generation
                .checked_add(1)
                .ok_or_else(|| AppError::Internal("ownership generation overflow".into()))?,
        };
        let lease_millis = duration_millis(config.lease_duration)?;
        let grant = sqlx::query_as::<_, LeaseGrant>(
            r#"INSERT INTO tenant_leases
                 (tenant_id, owner_cell, ownership_generation, expires_at, renewed_at)
               VALUES ($1, $2, $3, clock_timestamp() + $4 * INTERVAL '1 millisecond',
                       clock_timestamp())
               ON CONFLICT (tenant_id) DO UPDATE SET
                 owner_cell = EXCLUDED.owner_cell,
                 ownership_generation = EXCLUDED.ownership_generation,
                 expires_at = EXCLUDED.expires_at,
                 renewed_at = EXCLUDED.renewed_at
               RETURNING tenant_id, owner_cell, ownership_generation, expires_at"#,
        )
        .bind(tenant_id)
        .bind(cell_id)
        .bind(generation)
        .bind(lease_millis)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            r#"UPDATE tenants
               SET owner_cell = $2, ownership_generation = $3,
                   state = CASE WHEN $5 THEN 'RESTORING' WHEN $4 THEN 'ACTIVE' ELSE state END,
                   updated_at = clock_timestamp()
               WHERE id = $1"#,
        )
        .bind(tenant_id)
        .bind(cell_id)
        .bind(generation)
        .bind(activate)
        .bind(restoring)
        .execute(&mut *tx)
        .await?;
        if taking_ownership && (activate || require_assignment || restoring) {
            sqlx::query("SELECT pg_notify($1, $2)")
                .bind(crate::routing::ROUTING_CHANGED_CHANNEL)
                .bind(tenant_id.to_string())
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(grant)
    }

    pub async fn renew(
        &self,
        tenant_id: Uuid,
        cell_id: Uuid,
        generation: i64,
        config: LeaseConfig,
    ) -> Result<Option<LeaseGrant>, AppError> {
        let config = config.validate()?;
        let lease_millis = duration_millis(config.lease_duration)?;
        let grant = sqlx::query_as::<_, LeaseGrant>(
            r#"UPDATE tenant_leases lease
               SET expires_at = clock_timestamp() + $4 * INTERVAL '1 millisecond',
                   renewed_at = clock_timestamp()
               FROM tenants tenant, cells cell
               WHERE lease.tenant_id = $1
                 AND lease.owner_cell = $2
                 AND lease.ownership_generation = $3
                 AND tenant.id = lease.tenant_id
                 AND tenant.owner_cell = lease.owner_cell
                 AND tenant.ownership_generation = lease.ownership_generation
                 AND tenant.state NOT IN ('DELETED', 'FAILED')
                 AND cell.id = lease.owner_cell
                 AND cell.state <> 'OFFLINE'
               RETURNING lease.tenant_id, lease.owner_cell,
                         lease.ownership_generation, lease.expires_at"#,
        )
        .bind(tenant_id)
        .bind(cell_id)
        .bind(generation)
        .bind(lease_millis)
        .fetch_optional(&self.pool)
        .await?;
        Ok(grant)
    }

    pub async fn handoff(
        &self,
        tenant_id: Uuid,
        source_cell: Uuid,
        source_generation: i64,
        target_cell: Uuid,
        config: LeaseConfig,
    ) -> Result<LeaseGrant, AppError> {
        self.handoff_inner(tenant_id,source_cell,source_generation,target_cell,config,None).await
    }
    pub(crate) async fn handoff_move(&self,tenant_id:Uuid,source_cell:Uuid,source_generation:i64,target_cell:Uuid,config:LeaseConfig,job:Uuid)->Result<LeaseGrant,AppError> {
        self.handoff_inner(tenant_id,source_cell,source_generation,target_cell,config,Some(job)).await
    }
    async fn handoff_inner(&self,tenant_id:Uuid,source_cell:Uuid,source_generation:i64,target_cell:Uuid,config:LeaseConfig,move_id:Option<Uuid>)->Result<LeaseGrant,AppError> {
        let config = config.validate()?;
        if source_cell == target_cell {
            return Err(AppError::BadRequest(
                "Lease handoff requires a different target cell".into(),
            ));
        }

        let mut tx = self.pool.begin().await?;
        let target_state: Option<String> =
            sqlx::query_scalar("SELECT state FROM cells WHERE id = $1 FOR SHARE")
                .bind(target_cell)
                .fetch_optional(&mut *tx)
                .await?;
        if target_state.as_deref() != Some("ACTIVE") {
            return Err(AppError::Forbidden("Target cell is not active".into()));
        }

        let tenant: Option<(Option<Uuid>, i64, String)> = sqlx::query_as(
            "SELECT owner_cell, ownership_generation, state \
             FROM tenants WHERE id = $1 FOR UPDATE",
        )
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((tenant_owner, tenant_generation, tenant_state)) = tenant else {
            return Err(AppError::NotFound("Tenant not found".into()));
        };
        let lease: Option<(Uuid, i64)> = sqlx::query_as(
            "SELECT owner_cell, ownership_generation \
             FROM tenant_leases WHERE tenant_id = $1 FOR UPDATE",
        )
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await?;
        if tenant_state == "DELETED" || tenant_state == "FAILED" {
            return Err(AppError::Forbidden(
                "Tenant cannot transfer ownership".into(),
            ));
        }
        if tenant_owner != Some(source_cell)
            || tenant_generation != source_generation
            || lease != Some((source_cell, source_generation))
        {
            return Err(AppError::Conflict(
                "Source cell no longer owns this tenant generation".into(),
            ));
        }

        let fresh:bool=sqlx::query_scalar("SELECT expires_at>clock_timestamp()+$2*INTERVAL '1 millisecond' FROM tenant_leases WHERE tenant_id=$1")
            .bind(tenant_id).bind(duration_millis(config.fence_before_expiry)?).fetch_one(&mut *tx).await?;
        if !fresh {return Err(AppError::Forbidden("Source lease expired during handoff".into()));}
        if let Some(id)=move_id {
            let ready:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_moves WHERE id=$1 AND tenant_id=$2 AND source_cell=$3 AND target_cell=$4 AND source_ownership_generation=$5 AND phase='CUTOVER' AND (SELECT state FROM tenants WHERE id=$2)='CUTOVER' AND target_prepared_at IS NOT NULL AND target_image_checksum IS NOT NULL AND final_capture_number IS NOT NULL AND final_capture_number=target_capture_number AND source_replication_generation=(SELECT current_replication_generation FROM tenants WHERE id=$2) FOR UPDATE)")
                .bind(id).bind(tenant_id).bind(source_cell).bind(target_cell).bind(source_generation).fetch_one(&mut *tx).await?;
            if !ready {return Err(AppError::Conflict("Move target has not verified the final source capture".into()));}
        }
        let target_generation = source_generation
            .checked_add(1)
            .ok_or_else(|| AppError::Internal("ownership generation overflow".into()))?;
        let lease_millis = duration_millis(config.lease_duration)?;
        let grant = sqlx::query_as::<_, LeaseGrant>(
            r#"UPDATE tenant_leases
               SET owner_cell = $2,
                   ownership_generation = $3,
                   renewed_at = clock_timestamp(),
                   expires_at = clock_timestamp() + $4 * INTERVAL '1 millisecond'
               WHERE tenant_id = $1
               RETURNING tenant_id, owner_cell, ownership_generation, expires_at"#,
        )
        .bind(tenant_id)
        .bind(target_cell)
        .bind(target_generation)
        .bind(lease_millis)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            r#"UPDATE tenants
               SET owner_cell = $2,
                   ownership_generation = $3,
                   state = 'ACTIVE',
                   updated_at = clock_timestamp()
               WHERE id = $1"#,
        )
        .bind(tenant_id)
        .bind(target_cell)
        .bind(target_generation)
        .execute(&mut *tx)
        .await?;
        if let Some(id)=move_id {
            sqlx::query("UPDATE tenant_moves SET phase='VERIFYING',handed_off_at=clock_timestamp(),retain_source_until=clock_timestamp()+INTERVAL '7 days',updated_at=clock_timestamp() WHERE id=$1").bind(id).execute(&mut *tx).await?;
        }
        sqlx::query("SELECT pg_notify($1, $2)")
            .bind(crate::routing::ROUTING_CHANGED_CHANNEL)
            .bind(tenant_id.to_string())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(grant)
    }
}

#[derive(Clone)]
pub struct LeaseClient {
    repository: LeaseRepository,
    cell_id: Uuid,
    config: LeaseConfig,
    leases: Arc<RwLock<HashMap<Uuid, MeasuredLease>>>,
}

impl LeaseClient {
    pub fn new(pool: PgPool, cell_id: Uuid, config: LeaseConfig) -> Result<Self, AppError> {
        Ok(Self {
            repository: LeaseRepository::new(pool),
            cell_id,
            config: config.validate()?,
            leases: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub fn cell_id(&self) -> Uuid {
        self.cell_id
    }

    pub async fn acquire(&self, tenant_id: Uuid) -> Result<LeaseGrant, AppError> {
        let request_sent_at = Instant::now();
        let grant = self
            .repository
            .acquire(tenant_id, self.cell_id, self.config)
            .await?;
        self.store_acquired(grant.clone(), request_sent_at)?;
        Ok(grant)
    }

    pub async fn acquire_for_recovery(&self, tenant_id:Uuid)->Result<LeaseGrant,AppError> {
        let request_sent_at=Instant::now();
        let grant=self.repository.acquire_for_recovery(tenant_id,self.cell_id,self.config).await?;
        self.store_acquired(grant.clone(),request_sent_at)?;Ok(grant)
    }

    /// Called by cell startup, never by business request handling.
    pub async fn acquire_assigned(&self, tenant_id: Uuid) -> Result<LeaseGrant, AppError> {
        let request_sent_at = Instant::now();
        let grant = self
            .repository
            .acquire_assigned(tenant_id, self.cell_id, self.config)
            .await?;
        self.store_acquired(grant.clone(), request_sent_at)?;
        Ok(grant)
    }

    pub async fn acquire_for_provisioning(&self, tenant_id: Uuid) -> Result<LeaseGrant, AppError> {
        let request_sent_at = Instant::now();
        let grant = self
            .repository
            .acquire_for_provisioning(tenant_id, self.cell_id, self.config)
            .await?;
        self.store_acquired(grant.clone(), request_sent_at)?;
        Ok(grant)
    }

    pub async fn renew(&self, tenant_id: Uuid) -> Result<LeaseGrant, AppError> {
        let generation = self.current_generation(tenant_id)?;
        let request_sent_at = Instant::now();
        let grant = self
            .repository
            .renew(tenant_id, self.cell_id, generation, self.config)
            .await?;
        let Some(grant) = grant else {
            self.leases
                .write()
                .map_err(|_| AppError::Internal("lease state lock poisoned".into()))?
                .remove(&tenant_id);
            return Err(AppError::Forbidden("Ownership lease was fenced".into()));
        };
        self.store_renewed(grant.clone(), request_sent_at)?;
        Ok(grant)
    }

    pub fn ensure_writable(
        &self,
        tenant_id: Uuid,
        expected_generation: i64,
    ) -> Result<(), AppError> {
        let leases = self
            .leases
            .read()
            .map_err(|_| AppError::Internal("lease state lock poisoned".into()))?;
        let lease = leases
            .get(&tenant_id)
            .ok_or_else(|| AppError::Forbidden("Tenant has no ownership lease".into()))?;
        if lease.grant.ownership_generation != expected_generation {
            return Err(AppError::Forbidden(
                "Tenant ownership generation changed".into(),
            ));
        }
        if !lease.is_writable_at(Instant::now()) {
            return Err(AppError::Forbidden(
                "Tenant ownership lease is self-fenced".into(),
            ));
        }
        Ok(())
    }

    pub fn writable_generation(&self, tenant_id: Uuid) -> Result<i64, AppError> {
        let generation = self.current_generation(tenant_id)?;
        self.ensure_writable(tenant_id, generation)?;
        Ok(generation)
    }

    /// Fence the local writer, flush its remaining WAL, then atomically transfer ownership.
    ///
    /// The local lease stays fenced if flushing or the control-plane transaction fails.
    pub async fn handoff<F, Fut>(
        &self,
        tenant_id: Uuid,
        target_cell: Uuid,
        flush_remaining_wal: F,
    ) -> Result<LeaseGrant, AppError>
    where
        F: FnOnce(i64) -> Fut,
        Fut: Future<Output = Result<(), AppError>>,
    {
        let generation = self.fence_for_handoff(tenant_id)?;
        flush_remaining_wal(generation).await?;
        self.repository
            .handoff(
                tenant_id,
                self.cell_id,
                generation,
                target_cell,
                self.config,
            )
            .await
    }

    /// Reconcile a planned move after restart or an uncertain handoff. This
    /// never changes ownership: renewal matches the exact control generation.
    pub(crate) async fn resume_move_source(&self,tenant:Uuid,generation:i64)->Result<(),AppError> {
        let sent=Instant::now();
        let grant=self.repository.renew(tenant,self.cell_id,generation,self.config).await?
            .ok_or_else(||AppError::Forbidden("Move source no longer owns the recorded generation".into()))?;
        self.store_acquired(grant,sent)?;Ok(())
    }
    pub(crate) async fn handoff_move(&self,tenant:Uuid,target:Uuid,job:Uuid)->Result<LeaseGrant,AppError> {
        // The caller holds the tenant writer gate and has already uploaded and
        // verified its final capture. Fencing here prevents queued old writes.
        let generation=self.fence_for_handoff(tenant)?;
        self.repository.handoff_move(tenant,self.cell_id,generation,target,self.config,job).await
    }
    pub fn spawn_renewal(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.config.renewal_interval);
            interval.tick().await;
            loop {
                interval.tick().await;
                let tenant_ids = match self.leases.read() {
                    Ok(leases) => leases.keys().copied().collect::<Vec<_>>(),
                    Err(_) => {
                        tracing::error!("Ownership lease state lock poisoned");
                        continue;
                    }
                };
                let renewals = tenant_ids.into_iter().map(|tenant_id| {
                    let client = self.clone();
                    async move {
                        if let Err(error) = client.renew(tenant_id).await {
                            tracing::warn!(%tenant_id, %error, "Ownership lease renewal failed");
                        }
                    }
                });
                join_all(renewals).await;
            }
        })
    }

    fn current_generation(&self, tenant_id: Uuid) -> Result<i64, AppError> {
        self.leases
            .read()
            .map_err(|_| AppError::Internal("lease state lock poisoned".into()))?
            .get(&tenant_id)
            .map(|lease| lease.grant.ownership_generation)
            .ok_or_else(|| AppError::Forbidden("Tenant has no ownership lease".into()))
    }

    fn fence_for_handoff(&self, tenant_id: Uuid) -> Result<i64, AppError> {
        let mut leases = self
            .leases
            .write()
            .map_err(|_| AppError::Internal("lease state lock poisoned".into()))?;
        let lease = leases
            .remove(&tenant_id)
            .ok_or_else(|| AppError::Forbidden("Tenant has no ownership lease".into()))?;
        if !lease.is_writable_at(Instant::now()) {
            return Err(AppError::Forbidden(
                "Tenant ownership lease is self-fenced".into(),
            ));
        }
        Ok(lease.grant.ownership_generation)
    }

    fn store_acquired(&self, grant: LeaseGrant, request_sent_at: Instant) -> Result<(), AppError> {
        let measured = MeasuredLease::new(grant, request_sent_at, self.config);
        let tenant_id = measured.grant.tenant_id;
        self.leases
            .write()
            .map_err(|_| AppError::Internal("lease state lock poisoned".into()))?
            .insert(tenant_id, measured);
        Ok(())
    }

    fn store_renewed(&self, grant: LeaseGrant, request_sent_at: Instant) -> Result<(), AppError> {
        let mut leases = self
            .leases
            .write()
            .map_err(|_| AppError::Internal("lease state lock poisoned".into()))?;
        let still_held = leases.get(&grant.tenant_id).is_some_and(|current| {
            current.grant.ownership_generation == grant.ownership_generation
        });
        if !still_held {
            return Err(AppError::Forbidden(
                "Ownership lease was fenced for handoff".into(),
            ));
        }
        let measured = MeasuredLease::new(grant, request_sent_at, self.config);
        leases.insert(measured.grant.tenant_id, measured);
        Ok(())
    }
}

fn duration_millis(duration: Duration) -> Result<i64, AppError> {
    i64::try_from(duration.as_millis())
        .map_err(|_| AppError::Internal("ownership lease duration is too large".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant() -> LeaseGrant {
        LeaseGrant {
            tenant_id: Uuid::new_v4(),
            owner_cell: Uuid::new_v4(),
            ownership_generation: 7,
            expires_at: Utc::now(),
        }
    }

    #[test]
    fn defaults_match_the_ownership_decision() {
        let config = LeaseConfig::default();
        assert_eq!(config.lease_duration, Duration::from_secs(300));
        assert_eq!(config.renewal_interval, Duration::from_secs(60));
        assert_eq!(config.fence_before_expiry, Duration::from_secs(30));
        assert_eq!(config.reassignment_skew, Duration::from_secs(30));
        config.validate().unwrap();
    }

    #[test]
    fn measured_lease_self_fences_before_database_expiry() {
        let config = LeaseConfig {
            lease_duration: Duration::from_secs(10),
            renewal_interval: Duration::from_secs(2),
            fence_before_expiry: Duration::from_secs(3),
            reassignment_skew: Duration::from_secs(1),
        };
        let sent = Instant::now();
        let lease = MeasuredLease::new(grant(), sent, config);
        assert!(lease.is_writable_at(sent + Duration::from_secs(6)));
        assert!(!lease.is_writable_at(sent + Duration::from_secs(7)));
    }

    #[test]
    fn invalid_timings_are_rejected() {
        let config = LeaseConfig {
            lease_duration: Duration::from_secs(10),
            renewal_interval: Duration::from_secs(8),
            fence_before_expiry: Duration::from_secs(3),
            reassignment_skew: Duration::ZERO,
        };
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn in_flight_renewal_cannot_restore_a_handoff_fence() {
        let pool = PgPool::connect_lazy("postgres://localhost/test").unwrap();
        let client = LeaseClient::new(pool, Uuid::new_v4(), LeaseConfig::default()).unwrap();
        let grant = grant();
        client
            .store_acquired(grant.clone(), Instant::now())
            .unwrap();
        assert_eq!(
            client.fence_for_handoff(grant.tenant_id).unwrap(),
            grant.ownership_generation
        );
        assert!(client.store_renewed(grant.clone(), Instant::now()).is_err());
        assert!(client.writable_generation(grant.tenant_id).is_err());
    }
}

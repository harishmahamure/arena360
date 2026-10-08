use super::wal::Capture;
use crate::{error::AppError, tenancy::TenantDb};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub generation: Uuid,
    pub number: i64,
    pub object_key: String,
    pub capture: Capture,
    pub verified: bool,
}
#[async_trait]
pub trait Ledger: Send + Sync {
    async fn retention_tenants(&self) -> Result<Vec<Uuid>, AppError> { Ok(vec![]) }
    async fn retain(&self, _db: &TenantDb, _store: &dyn object_store::ObjectStore) -> Result<usize, AppError> { Err(AppError::Internal("Retention registry unavailable".into())) }
    async fn daily_tenants(&self) -> Result<Vec<Uuid>, AppError> {
        Err(AppError::Internal("Snapshot registry unavailable".into()))
    }
    async fn snapshot(
        &self,
        _db: Arc<TenantDb>,
        _store: &dyn object_store::ObjectStore,
        _keys: &super::crypto::TenantKeys,
        _kind: super::snapshot::Kind,
    ) -> Result<(), AppError> {
        Err(AppError::Internal(
            "Snapshot provider is unavailable".into(),
        ))
    }
    async fn baseline(
        &self,
        db: Arc<TenantDb>,
        _generation: Uuid,
        store: &dyn object_store::ObjectStore,
        keys: &super::crypto::TenantKeys,
    ) -> Result<(), AppError> {
        self.snapshot(db, store, keys, super::snapshot::Kind::Baseline)
            .await
    }
    async fn reserve(&self, db: &TenantDb, capture: &Capture) -> Result<Segment, AppError>;
    async fn manifest(
        &self,
        db: &TenantDb,
        segment: &Segment,
        checksum: &str,
        size: i64,
    ) -> Result<Vec<u8>, AppError>;
    async fn verified(
        &self,
        db: &TenantDb,
        segment: &Segment,
        checksum: &str,
        size: i64,
    ) -> Result<(), AppError>;
}
pub struct PostgresLedger {
    pub pool: PgPool,
    pub cell_id: Uuid,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationReason {
    LeaseChange,
    Restore,
    WalGap,
}
impl GenerationReason {
    fn name(self) -> &'static str {
        match self {
            Self::LeaseChange => "LEASE_CHANGE",
            Self::Restore => "RESTORE",
            Self::WalGap => "WAL_GAP",
        }
    }
    fn sealed_state(self) -> &'static str {
        match self {
            Self::LeaseChange => "SEALED",
            Self::Restore => "RESTORED",
            Self::WalGap => "GAPPED",
        }
    }
}
impl PostgresLedger {
    pub async fn ensure_generation(&self, db: &TenantDb) -> Result<Uuid, AppError> {
        let mut tx = self.pool.begin().await?;
        let current = self.lock_owner(db, &mut tx).await?;
        if let Some(id) = current {
            let matching:bool=sqlx::query_scalar("SELECT ownership_generation=$2 AND state='ACTIVE' FROM replication_generations WHERE id=$1").bind(id).bind(db.ownership_generation()).fetch_one(&mut *tx).await?;
            if matching {
                db.ensure_current_owner()?;
                tx.commit().await?;
                return Ok(id);
            }
        }
        let id = self
            .rotate_locked(db, current, GenerationReason::LeaseChange, None, &mut tx)
            .await?;
        db.ensure_current_owner()?;
        tx.commit().await?;
        Ok(id)
    }
    /// Caller persists the transition ID before starting restore/gap handling.
    /// Repeating the same transition is idempotent; a superseded transition
    /// cannot make an old generation current again.
    pub async fn start_generation(
        &self,
        db: &TenantDb,
        reason: GenerationReason,
        transition: Uuid,
    ) -> Result<Uuid, AppError> {
        let mut tx = self.pool.begin().await?;
        let current = self.lock_owner(db, &mut tx).await?;
        if let Some(row)=sqlx::query("SELECT id,ownership_generation,start_reason FROM replication_generations WHERE tenant_id=$1 AND transition_id=$2").bind(db.tenant_id()).bind(transition).fetch_optional(&mut *tx).await? {
            let id:Uuid=row.get(0);
            if current!=Some(id)||row.get::<i64,_>(1)!=db.ownership_generation()||row.get::<String,_>(2)!=reason.name() {return Err(AppError::Conflict("Replication transition was superseded or has different ownership/reason".into()));}
            db.ensure_current_owner()?; tx.commit().await?;return Ok(id);
        }
        let id = self
            .rotate_locked(db, current, reason, Some(transition), &mut tx)
            .await?;
        db.ensure_current_owner()?;
        tx.commit().await?;
        Ok(id)
    }
    async fn rotate_locked(
        &self,
        db: &TenantDb,
        current: Option<Uuid>,
        reason: GenerationReason,
        transition: Option<Uuid>,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Uuid, AppError> {
        let id = Uuid::new_v4();
        if let Some(old) = current {
            sqlx::query("UPDATE replication_generations SET state=$2,sealed_at=NOW() WHERE id=$1 AND state='ACTIVE'").bind(old).bind(reason.sealed_state()).execute(&mut **tx).await?;
        }
        let key = format!(
            "tenants/{}/replication/generations/{id}/manifest.json",
            db.tenant_id()
        );
        sqlx::query("INSERT INTO replication_generations(id,tenant_id,ownership_generation,manifest_object_key,start_reason,transition_id) VALUES($1,$2,$3,$4,$5,$6)").bind(id).bind(db.tenant_id()).bind(db.ownership_generation()).bind(key).bind(reason.name()).bind(transition).execute(&mut **tx).await?;
        sqlx::query(
            "UPDATE tenants SET current_replication_generation=$2,updated_at=NOW() WHERE id=$1",
        )
        .bind(db.tenant_id())
        .bind(id)
        .execute(&mut **tx)
        .await?;
        Ok(id)
    }
    pub(crate) async fn lock_owner(
        &self,
        db: &TenantDb,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Option<Uuid>, AppError> {
        db.ensure_current_owner()?;
        let row=sqlx::query("SELECT t.current_replication_generation FROM tenants t JOIN tenant_leases l ON l.tenant_id=t.id WHERE t.id=$1 AND t.owner_cell=$2 AND l.owner_cell=$2 AND t.ownership_generation=$3 AND l.ownership_generation=$3 AND l.expires_at>clock_timestamp()+INTERVAL '30 seconds' AND t.state <> 'DELETED' FOR UPDATE OF t,l")
            .bind(db.tenant_id()).bind(self.cell_id).bind(db.ownership_generation()).fetch_optional(&mut **tx).await?
            .ok_or_else(||AppError::Forbidden("Replication ownership lease expired or changed".into()))?;
        db.ensure_current_owner()?;
        let fresh:bool=sqlx::query_scalar("SELECT expires_at>clock_timestamp()+INTERVAL '30 seconds' FROM tenant_leases WHERE tenant_id=$1").bind(db.tenant_id()).fetch_one(&mut **tx).await?;
        if !fresh {
            return Err(AppError::Forbidden(
                "Replication lease expired while waiting for control lock".into(),
            ));
        }
        Ok(row.get(0))
    }
    async fn assert_generation(
        &self,
        db: &TenantDb,
        segment: &Segment,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), AppError> {
        if self.lock_owner(db, tx).await? != Some(segment.generation) {
            return Err(AppError::Conflict("Replication generation changed".into()));
        }
        Ok(())
    }
}
#[async_trait]
impl Ledger for PostgresLedger {
    async fn retention_tenants(&self) -> Result<Vec<Uuid>, AppError> {
        Ok(sqlx::query_scalar("SELECT id FROM tenants WHERE owner_cell=$1 AND state='ACTIVE' ORDER BY id").bind(self.cell_id).fetch_all(&self.pool).await?)
    }
    async fn retain(&self, db: &TenantDb, store: &dyn object_store::ObjectStore) -> Result<usize, AppError> {super::retention::run(db,self,store).await}

    async fn daily_tenants(&self) -> Result<Vec<Uuid>, AppError> {
        Ok(sqlx::query_scalar("SELECT t.id FROM tenants t WHERE t.owner_cell=$1 AND t.state='ACTIVE' AND NOT EXISTS(SELECT 1 FROM snapshot_manifests s WHERE s.generation_id=t.current_replication_generation AND s.kind='DAILY' AND s.verified_at IS NOT NULL AND s.retired_at IS NULL AND s.snapshot_at>clock_timestamp()-INTERVAL '1 day') ORDER BY t.id").bind(self.cell_id).fetch_all(&self.pool).await?)
    }
    async fn snapshot(
        &self,
        db: Arc<TenantDb>,
        store: &dyn object_store::ObjectStore,
        keys: &super::crypto::TenantKeys,
        kind: super::snapshot::Kind,
    ) -> Result<(), AppError> {
        super::snapshot::take(db, self, store, keys, kind).await
    }
    async fn baseline(
        &self,
        db: Arc<TenantDb>,
        generation: Uuid,
        store: &dyn object_store::ObjectStore,
        keys: &super::crypto::TenantKeys,
    ) -> Result<(), AppError> {
        let ready:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM snapshot_manifests WHERE generation_id=$1 AND verified_at IS NOT NULL AND retired_at IS NULL AND source_checksum_sha256 IS NOT NULL)").bind(generation).fetch_one(&self.pool).await?;
        if ready {
            return Ok(());
        }
        self.snapshot(db, store, keys, super::snapshot::Kind::Baseline)
            .await
    }
    async fn reserve(&self, db: &TenantDb, capture: &Capture) -> Result<Segment, AppError> {
        if capture.ownership_generation != db.ownership_generation() {
            return Err(AppError::Conflict(
                "Spool belongs to another ownership generation".into(),
            ));
        }
        let mut tx = self.pool.begin().await?;
        let current = self.lock_owner(db, &mut tx).await?;
        let matching = if let Some(id) = current {
            sqlx::query_scalar::<_,bool>("SELECT ownership_generation=$2 AND state='ACTIVE' FROM replication_generations WHERE id=$1").bind(id).bind(db.ownership_generation()).fetch_optional(&mut *tx).await?.unwrap_or(false)
        } else {
            false
        };
        let mut generation = if matching {
            current.unwrap()
        } else {
            self.rotate_locked(db, current, GenerationReason::LeaseChange, None, &mut tx)
                .await?
        };
        let existing=sqlx::query("SELECT segment_number,object_key,verified_at IS NOT NULL AS verified FROM replication_segments WHERE generation_id=$1 AND source_checksum=$2").bind(generation).bind(&capture.checksum).fetch_optional(&mut *tx).await?;
        let segment = if let Some(row) = existing {
            Segment {
                generation,
                number: row.get(0),
                object_key: row.get(1),
                capture: capture.clone(),
                verified: row.get(2),
            }
        } else {
            let previous:Option<sqlx::types::Json<Capture>>=sqlx::query_scalar("SELECT capture FROM replication_segments WHERE generation_id=$1 ORDER BY segment_number DESC LIMIT 1").bind(generation).fetch_optional(&mut *tx).await?;
            if previous.as_ref().is_some_and(|p| {
                p.range_end().checked_add(1) != Some(capture.capture_number)
                    || (p.version == 1
                        && capture.version == 1
                        && p.salt == capture.salt
                        && capture.frames < p.frames)
            }) {
                // A missing/reordered capture or backwards frame boundary is
                // conservative evidence of a gap. Never append it to the old
                // lineage; API-0050 supplies the new generation's baseline.
                generation = self
                    .rotate_locked(
                        db,
                        Some(generation),
                        GenerationReason::WalGap,
                        None,
                        &mut tx,
                    )
                    .await?;
            }
            let number:i64=sqlx::query_scalar("SELECT COALESCE(MAX(segment_number),0)+1 FROM replication_segments WHERE generation_id=$1").bind(generation).fetch_one(&mut *tx).await?;
            let key = format!(
                "tenants/{}/replication/generations/{generation}/wal/{number:010}.wal.zst",
                db.tenant_id()
            );
            sqlx::query("INSERT INTO replication_segments(generation_id,segment_number,source_checksum,object_key,capture) VALUES($1,$2,$3,$4,$5)").bind(generation).bind(number).bind(&capture.checksum).bind(&key).bind(sqlx::types::Json(capture)).execute(&mut *tx).await?;
            Segment {
                generation,
                number,
                object_key: key,
                capture: capture.clone(),
                verified: false,
            }
        };
        db.ensure_current_owner()?;
        tx.commit().await?;
        Ok(segment)
    }
    async fn manifest(
        &self,
        db: &TenantDb,
        segment: &Segment,
        checksum: &str,
        size: i64,
    ) -> Result<Vec<u8>, AppError> {
        let mut tx = self.pool.begin().await?;
        self.assert_generation(db, segment, &mut tx).await?;
        let previous:Vec<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_build_object('number',segment_number,'object_key',object_key,'capture',capture,'checksum_sha256',checksum_sha256,'encrypted_size_bytes',encrypted_size_bytes,'retired_at',retired_at) FROM replication_segments WHERE generation_id=$1 AND verified_at IS NOT NULL ORDER BY segment_number").bind(segment.generation).fetch_all(&mut *tx).await?;
        if previous.len() as i64 != segment.number - 1 {
            return Err(AppError::Conflict(
                "Earlier WAL segment is not verified".into(),
            ));
        }
        let mut list = previous;
        list.push(serde_json::json!({"number":segment.number,"object_key":segment.object_key,"capture":segment.capture,"checksum_sha256":checksum,"encrypted_size_bytes":size}));
        let snapshots:Vec<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'object_key',object_key,'kind',kind,'schema_version',schema_version,'snapshot_at',snapshot_at,'capture_number',capture_number,'event_sequence',event_sequence,'source_checksum_sha256',source_checksum_sha256,'checksum_sha256',checksum_sha256,'encrypted_size_bytes',encrypted_size_bytes,'retired_at',retired_at) FROM snapshot_manifests WHERE generation_id=$1 AND verified_at IS NOT NULL ORDER BY snapshot_at,id").bind(segment.generation).fetch_all(&mut *tx).await?;
        let revision:i64=sqlx::query_scalar("UPDATE replication_generations SET manifest_revision=manifest_revision+1 WHERE id=$1 RETURNING manifest_revision").bind(segment.generation).fetch_one(&mut *tx).await?;
        let reason: String =
            sqlx::query_scalar("SELECT start_reason FROM replication_generations WHERE id=$1")
                .bind(segment.generation)
                .fetch_one(&mut *tx)
                .await?;
        let bytes=serde_json::to_vec(&serde_json::json!({"version":1,"tenant_id":db.tenant_id(),"generation_id":segment.generation,"ownership_generation":db.ownership_generation(),"start_reason":reason,"revision":revision,"delta":{"segment":list.last()},"segments":list,"snapshots":snapshots})).map_err(|e|AppError::Internal(e.to_string()))?;
        db.ensure_current_owner()?;
        tx.commit().await?;
        Ok(bytes)
    }
    async fn verified(
        &self,
        db: &TenantDb,
        segment: &Segment,
        checksum: &str,
        size: i64,
    ) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await?;
        self.assert_generation(db, segment, &mut tx).await?;
        sqlx::query("UPDATE replication_segments SET checksum_sha256=$3,encrypted_size_bytes=$4,verified_at=NOW() WHERE generation_id=$1 AND segment_number=$2 AND verified_at IS NULL").bind(segment.generation).bind(segment.number).bind(checksum).bind(size).execute(&mut *tx).await?;
        db.ensure_current_owner()?;
        tx.commit().await?;
        Ok(())
    }
}

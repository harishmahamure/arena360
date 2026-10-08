use super::wal::Capture;
use crate::{error::AppError, tenancy::TenantDb};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
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
impl PostgresLedger {
    async fn lock_owner(
        &self,
        db: &TenantDb,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Option<Uuid>, AppError> {
        db.ensure_current_owner()?;
        let row=sqlx::query("SELECT t.current_replication_generation FROM tenants t JOIN tenant_leases l ON l.tenant_id=t.id WHERE t.id=$1 AND t.owner_cell=$2 AND l.owner_cell=$2 AND t.ownership_generation=$3 AND l.ownership_generation=$3 AND l.expires_at>NOW()+INTERVAL '30 seconds' AND t.state <> 'DELETED' FOR UPDATE OF t,l")
            .bind(db.tenant_id()).bind(self.cell_id).bind(db.ownership_generation()).fetch_optional(&mut **tx).await?
            .ok_or_else(||AppError::Forbidden("Replication ownership lease expired or changed".into()))?;
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
        let generation = if matching {
            current.unwrap()
        } else {
            let id = Uuid::new_v4();
            let key = format!(
                "tenants/{}/replication/generations/{id}/manifest.json",
                db.tenant_id()
            );
            if let Some(old) = current {
                sqlx::query("UPDATE replication_generations SET state='SEALED',sealed_at=NOW() WHERE id=$1 AND state='ACTIVE'").bind(old).execute(&mut *tx).await?;
            }
            sqlx::query("INSERT INTO replication_generations(id,tenant_id,ownership_generation,manifest_object_key) VALUES($1,$2,$3,$4)").bind(id).bind(db.tenant_id()).bind(db.ownership_generation()).bind(key).execute(&mut *tx).await?;
            sqlx::query("UPDATE tenants SET current_replication_generation=$2 WHERE id=$1")
                .bind(db.tenant_id())
                .bind(id)
                .execute(&mut *tx)
                .await?;
            id
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
        let previous:Vec<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_build_object('number',segment_number,'object_key',object_key,'capture',capture,'checksum_sha256',checksum_sha256,'encrypted_size_bytes',encrypted_size_bytes) FROM replication_segments WHERE generation_id=$1 AND verified_at IS NOT NULL ORDER BY segment_number").bind(segment.generation).fetch_all(&mut *tx).await?;
        if previous.len() as i64 != segment.number - 1 {
            return Err(AppError::Conflict(
                "Earlier WAL segment is not verified".into(),
            ));
        }
        let mut list = previous;
        list.push(serde_json::json!({"number":segment.number,"object_key":segment.object_key,"capture":segment.capture,"checksum_sha256":checksum,"encrypted_size_bytes":size}));
        let bytes=serde_json::to_vec(&serde_json::json!({"version":1,"tenant_id":db.tenant_id(),"generation_id":segment.generation,"ownership_generation":db.ownership_generation(),"segments":list})).map_err(|e|AppError::Internal(e.to_string()))?;
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
        tx.commit().await?;
        Ok(())
    }
}

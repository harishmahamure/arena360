use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::AppError;

const ISSUER: &str = "arena360-control";
const AUDIENCE: &str = "arena360-cell";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TenantEntitlement {
    pub tenant_id: Uuid,
    pub timezone: String,
    pub revision: i64,
    pub entitlements: serde_json::Value,
    pub valid_until: DateTime<Utc>,
    pub grace_until: DateTime<Utc>,
    pub iss: String,
    pub aud: String,
    pub iat: i64,
    pub exp: i64,
}

pub fn sign(
    secret: &[u8],
    tenant_id: Uuid,
    timezone: String,
    revision: i64,
    entitlements: serde_json::Value,
    valid_until: DateTime<Utc>,
    grace_until: DateTime<Utc>,
) -> Result<String, AppError> {
    let now = Utc::now();
    let claims = TenantEntitlement {
        tenant_id,
        timezone,
        revision,
        entitlements,
        valid_until,
        grace_until,
        iss: ISSUER.into(),
        aud: AUDIENCE.into(),
        iat: now.timestamp(),
        // A cell may operate through a control-plane outage until the signed grace period.
        exp: grace_until.timestamp(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
    .map_err(AppError::Jwt)
}

pub fn verify(secret: &[u8], token: &str) -> Result<TenantEntitlement, AppError> {
    let mut validation = Validation::default();
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[AUDIENCE]);
    decode::<TenantEntitlement>(token, &DecodingKey::from_secret(secret), &validation)
        .map(|data| data.claims)
        .map_err(AppError::Jwt)
}

#[derive(Clone)]
pub struct EntitlementCache {
    secret: Arc<[u8]>,
    entries: Arc<RwLock<HashMap<Uuid, TenantEntitlement>>>,
}

impl EntitlementCache {
    pub fn new(secret: impl Into<Arc<[u8]>>) -> Self {
        Self {
            secret: secret.into(),
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn update(&self, token: &str) -> Result<TenantEntitlement, AppError> {
        let claims = verify(&self.secret, token)?;
        claims
            .timezone
            .parse::<chrono_tz::Tz>()
            .map_err(|_| AppError::BadRequest("Invalid tenant time zone".into()))?;
        let mut entries = self.entries.write().await;
        if entries
            .get(&claims.tenant_id)
            .is_some_and(|current| current.revision > claims.revision)
        {
            return Err(AppError::Conflict(
                "Stale tenant entitlement revision".into(),
            ));
        }
        entries.insert(claims.tenant_id, claims.clone());
        Ok(claims)
    }

    pub async fn get(&self, tenant_id: Uuid) -> Result<TenantEntitlement, AppError> {
        let claims = self
            .entries
            .read()
            .await
            .get(&tenant_id)
            .cloned()
            .ok_or_else(|| AppError::Forbidden("Tenant entitlement unavailable".into()))?;
        if claims.grace_until <= Utc::now() {
            return Err(AppError::Forbidden("Tenant entitlement expired".into()));
        }
        Ok(claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::json;

    const SECRET: &[u8] = b"control-entitlement-test-secret-32-bytes";

    fn token(tenant: Uuid, revision: i64) -> String {
        let now = Utc::now();
        sign(
            SECRET,
            tenant,
            "Asia/Kolkata".into(),
            revision,
            json!({"maxDevices": 20}),
            now + Duration::hours(1),
            now + Duration::days(7),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn cached_entitlement_needs_no_database() {
        let tenant = Uuid::new_v4();
        let cache = EntitlementCache::new(SECRET);
        cache.update(&token(tenant, 1)).await.unwrap();

        let cached = cache.get(tenant).await.unwrap();
        assert_eq!(cached.timezone, "Asia/Kolkata");
        assert_eq!(cached.entitlements["maxDevices"], 20);
    }

    #[tokio::test]
    async fn rejects_tampering_and_revision_rollback() {
        let tenant = Uuid::new_v4();
        let cache = EntitlementCache::new(SECRET);
        cache.update(&token(tenant, 2)).await.unwrap();
        assert!(cache.update(&token(tenant, 1)).await.is_err());

        let mut tampered = token(tenant, 3);
        tampered.push('x');
        assert!(cache.update(&tampered).await.is_err());
    }
}

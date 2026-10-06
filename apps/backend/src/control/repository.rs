use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Tenant {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub owner_cell: Option<Uuid>,
    pub ownership_generation: i64,
    pub schema_version: i64,
    pub state: String,
    pub timezone: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTenant {
    pub slug: String,
    pub name: String,
    pub timezone: String,
    pub owner_cell: Option<Uuid>,
    pub subscription_plan: String,
    pub entitlements: serde_json::Value,
    pub trial_ends_at: DateTime<Utc>,
    pub entitlement_grace_until: DateTime<Utc>,
}

#[derive(Clone)]
pub struct Repository {
    pool: PgPool,
}

impl Repository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn create_tenant(&self, input: CreateTenant) -> Result<Tenant, AppError> {
        input
            .timezone
            .parse::<chrono_tz::Tz>()
            .map_err(|_| AppError::BadRequest("timezone must be an IANA time zone".into()))?;
        if input.trial_ends_at <= Utc::now() || input.entitlement_grace_until < input.trial_ends_at
        {
            return Err(AppError::BadRequest(
                "trial and entitlement grace times are invalid".into(),
            ));
        }
        if !input.entitlements.is_object() {
            return Err(AppError::BadRequest(
                "entitlements must be a JSON object".into(),
            ));
        }

        let mut tx = self.pool.begin().await?;
        let tenant = sqlx::query_as::<_, Tenant>(
            r#"INSERT INTO tenants (slug, name, owner_cell, state, timezone)
               VALUES ($1, $2, $3, 'PROVISIONING', $4)
               RETURNING id, slug, name, owner_cell, ownership_generation,
                         schema_version, state, timezone"#,
        )
        .bind(input.slug)
        .bind(input.name)
        .bind(input.owner_cell)
        .bind(input.timezone)
        .fetch_one(&mut *tx)
        .await?;

        let now = Utc::now();
        sqlx::query(
            r#"INSERT INTO subscriptions
                 (tenant_id, plan_code, status, starts_at, ends_at)
               VALUES ($1, $2, 'TRIAL', $3, $4)"#,
        )
        .bind(tenant.id)
        .bind(input.subscription_plan)
        .bind(now)
        .bind(input.trial_ends_at)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"INSERT INTO licenses
                 (tenant_id, status, revision, entitlements, valid_from, valid_until, grace_until)
               VALUES ($1, 'ACTIVE', 1, $2, $3, $4, $5)"#,
        )
        .bind(tenant.id)
        .bind(input.entitlements)
        .bind(now)
        .bind(input.trial_ends_at)
        .bind(input.entitlement_grace_until)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(tenant)
    }

    pub async fn create_or_resume_tenant(&self, input: CreateTenant) -> Result<Tenant, AppError> {
        let expected_slug = input.slug.clone();
        let expected_name = input.name.clone();
        let expected_timezone = input.timezone.clone();
        let expected_owner = input.owner_cell;
        match self.create_tenant(input).await {
            Ok(tenant) => Ok(tenant),
            Err(AppError::Database(error)) if is_unique_violation(&error) => {
                let tenant = self
                    .tenant_by_slug(&expected_slug)
                    .await?
                    .ok_or_else(|| AppError::Conflict("Tenant slug already exists".into()))?;
                if tenant.name != expected_name
                    || tenant.timezone != expected_timezone
                    || expected_owner.is_some_and(|owner| {
                        tenant.owner_cell.is_some_and(|current| current != owner)
                    })
                    || !matches!(tenant.state.as_str(), "PROVISIONING" | "ACTIVE")
                {
                    return Err(AppError::Conflict(
                        "Tenant slug belongs to a different provisioning request".into(),
                    ));
                }
                Ok(tenant)
            }
            Err(error) => Err(error),
        }
    }

    pub async fn tenant_by_slug(&self, slug: &str) -> Result<Option<Tenant>, AppError> {
        Ok(sqlx::query_as::<_, Tenant>(
            r#"SELECT id, slug, name, owner_cell, ownership_generation,
                      schema_version, state, timezone
               FROM tenants WHERE slug = $1"#,
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn finalize_provisioning(
        &self,
        tenant_id: Uuid,
        cell_id: Uuid,
        ownership_generation: i64,
        schema_version: i64,
    ) -> Result<Tenant, AppError> {
        let mut tx = self.pool.begin().await?;
        let tenant = sqlx::query_as::<_, Tenant>(
            r#"UPDATE tenants
               SET schema_version = $4, state = 'ACTIVE', updated_at = clock_timestamp()
               WHERE id = $1
                 AND owner_cell = $2
                 AND ownership_generation = $3
                 AND state IN ('PROVISIONING', 'ACTIVE')
                 AND schema_version <= $4
               RETURNING id, slug, name, owner_cell, ownership_generation,
                         schema_version, state, timezone"#,
        )
        .bind(tenant_id)
        .bind(cell_id)
        .bind(ownership_generation)
        .bind(schema_version)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::Conflict("Tenant ownership changed during provisioning".into()))?;
        sqlx::query("SELECT pg_notify($1, $2)")
            .bind(crate::routing::ROUTING_CHANGED_CHANNEL)
            .bind(tenant_id.to_string())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(tenant)
    }

    pub async fn signed_entitlement(
        &self,
        tenant_id: Uuid,
        signing_secret: &[u8],
    ) -> Result<String, AppError> {
        let row: Option<(String, i64, serde_json::Value, DateTime<Utc>, DateTime<Utc>)> =
            sqlx::query_as(
                r#"SELECT t.timezone, l.revision, l.entitlements, l.valid_until, l.grace_until
                   FROM tenants t
                   JOIN licenses l ON l.tenant_id = t.id
                   WHERE t.id = $1 AND t.state NOT IN ('DELETED', 'FAILED')
                     AND l.status = 'ACTIVE' AND l.grace_until > NOW()
                   ORDER BY l.revision DESC LIMIT 1"#,
            )
            .bind(tenant_id)
            .fetch_optional(&self.pool)
            .await?;
        let (timezone, revision, entitlements, valid_until, grace_until) =
            row.ok_or_else(|| AppError::Forbidden("No active tenant entitlement".into()))?;
        super::entitlement::sign(
            signing_secret,
            tenant_id,
            timezone,
            revision,
            entitlements,
            valid_until,
            grace_until,
        )
    }
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|error| error.code())
        .is_some_and(|code| code == "23505")
}

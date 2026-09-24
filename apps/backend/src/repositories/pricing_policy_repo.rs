use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{PricingRuleSet, PricingRuleVersion};
use crate::realtime::OutboxService;

#[derive(Clone)]
pub struct PricingPolicyRepository {
    pool: PgPool,
}

impl PricingPolicyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_sets(
        &self,
        organization_id: Uuid,
        location_id: Option<Uuid>,
    ) -> Result<Vec<PricingRuleSet>, AppError> {
        Ok(sqlx::query_as::<_, PricingRuleSet>(
            r#"SELECT id, "organizationId" AS organization_id,
                      "locationId" AS location_id, name, description,
                      "activeVersionId" AS active_version_id,
                      "createdBy" AS created_by, "createdAt" AS created_at,
                      "updatedAt" AS updated_at
               FROM pricing_rule_sets
               WHERE "organizationId" = $1
                 AND ($2::uuid IS NULL OR "locationId" = $2)
               ORDER BY name, id"#,
        )
        .bind(organization_id)
        .bind(location_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn get_set(
        &self,
        organization_id: Uuid,
        set_id: Uuid,
    ) -> Result<PricingRuleSet, AppError> {
        sqlx::query_as::<_, PricingRuleSet>(
            r#"SELECT id, "organizationId" AS organization_id,
                      "locationId" AS location_id, name, description,
                      "activeVersionId" AS active_version_id,
                      "createdBy" AS created_by, "createdAt" AS created_at,
                      "updatedAt" AS updated_at
               FROM pricing_rule_sets
               WHERE id = $1 AND "organizationId" = $2"#,
        )
        .bind(set_id)
        .bind(organization_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Pricing rule set {set_id} not found")))
    }

    pub async fn create_set(
        &self,
        organization_id: Uuid,
        location_id: Option<Uuid>,
        name: &str,
        description: Option<&str>,
        policy: &serde_json::Value,
        actor_id: Uuid,
    ) -> Result<(PricingRuleSet, PricingRuleVersion), AppError> {
        let mut tx = self.pool.begin().await?;
        let set = sqlx::query_as::<_, PricingRuleSet>(
            r#"INSERT INTO pricing_rule_sets
                 ("organizationId", "locationId", name, description, "createdBy")
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id, "organizationId" AS organization_id,
                         "locationId" AS location_id, name, description,
                         "activeVersionId" AS active_version_id,
                         "createdBy" AS created_by, "createdAt" AS created_at,
                         "updatedAt" AS updated_at"#,
        )
        .bind(organization_id)
        .bind(location_id)
        .bind(name)
        .bind(description)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await?;
        let version = sqlx::query_as::<_, PricingRuleVersion>(
            r#"INSERT INTO pricing_rule_versions
                 ("ruleSetId", version, status, policy, "createdBy")
               VALUES ($1, 1, 'draft', $2, $3)
               RETURNING id, "ruleSetId" AS rule_set_id, version,
                         status::text AS status, policy,
                         "simulationHash" AS simulation_hash,
                         "validatedAt" AS validated_at, "effectiveAt" AS effective_at,
                         "publishedAt" AS published_at, "createdBy" AS created_by,
                         "publishedBy" AS published_by, "createdAt" AS created_at"#,
        )
        .bind(set.id)
        .bind(policy)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((set, version))
    }

    pub async fn create_version(
        &self,
        organization_id: Uuid,
        set_id: Uuid,
        policy: &serde_json::Value,
        actor_id: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        let mut tx = self.pool.begin().await?;
        let owned: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT id FROM pricing_rule_sets
               WHERE id = $1 AND "organizationId" = $2
               FOR UPDATE"#,
        )
        .bind(set_id)
        .bind(organization_id)
        .fetch_optional(&mut *tx)
        .await?;
        if owned.is_none() {
            return Err(AppError::NotFound(format!(
                "Pricing rule set {set_id} not found"
            )));
        }
        let version = sqlx::query_as::<_, PricingRuleVersion>(
            r#"INSERT INTO pricing_rule_versions
                 ("ruleSetId", version, status, policy, "createdBy")
               SELECT $1, COALESCE(MAX(version), 0) + 1, 'draft', $2, $3
               FROM pricing_rule_versions WHERE "ruleSetId" = $1
               RETURNING id, "ruleSetId" AS rule_set_id, version,
                         status::text AS status, policy,
                         "simulationHash" AS simulation_hash,
                         "validatedAt" AS validated_at, "effectiveAt" AS effective_at,
                         "publishedAt" AS published_at, "createdBy" AS created_by,
                         "publishedBy" AS published_by, "createdAt" AS created_at"#,
        )
        .bind(set_id)
        .bind(policy)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(version)
    }

    pub async fn versions(
        &self,
        organization_id: Uuid,
        set_id: Uuid,
    ) -> Result<Vec<PricingRuleVersion>, AppError> {
        self.get_set(organization_id, set_id).await?;
        Ok(sqlx::query_as::<_, PricingRuleVersion>(
            r#"SELECT id, "ruleSetId" AS rule_set_id, version,
                      status::text AS status, policy,
                      "simulationHash" AS simulation_hash,
                      "validatedAt" AS validated_at, "effectiveAt" AS effective_at,
                      "publishedAt" AS published_at, "createdBy" AS created_by,
                      "publishedBy" AS published_by, "createdAt" AS created_at
               FROM pricing_rule_versions
               WHERE "ruleSetId" = $1
               ORDER BY version DESC"#,
        )
        .bind(set_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn get_version(
        &self,
        organization_id: Uuid,
        set_id: Uuid,
        version_id: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        self.get_set(organization_id, set_id).await?;
        sqlx::query_as::<_, PricingRuleVersion>(
            r#"SELECT id, "ruleSetId" AS rule_set_id, version,
                      status::text AS status, policy,
                      "simulationHash" AS simulation_hash,
                      "validatedAt" AS validated_at, "effectiveAt" AS effective_at,
                      "publishedAt" AS published_at, "createdBy" AS created_by,
                      "publishedBy" AS published_by, "createdAt" AS created_at
               FROM pricing_rule_versions
               WHERE id = $1 AND "ruleSetId" = $2"#,
        )
        .bind(version_id)
        .bind(set_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Pricing rule version {version_id} not found")))
    }

    pub async fn mark_validated(
        &self,
        organization_id: Uuid,
        set_id: Uuid,
        version_id: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        self.get_version(organization_id, set_id, version_id)
            .await?;
        Ok(sqlx::query_as::<_, PricingRuleVersion>(
            r#"UPDATE pricing_rule_versions
               SET status = 'validated', "validatedAt" = NOW()
               WHERE id = $1 AND status IN ('draft', 'validated')
               RETURNING id, "ruleSetId" AS rule_set_id, version,
                         status::text AS status, policy,
                         "simulationHash" AS simulation_hash,
                         "validatedAt" AS validated_at, "effectiveAt" AS effective_at,
                         "publishedAt" AS published_at, "createdBy" AS created_by,
                         "publishedBy" AS published_by, "createdAt" AS created_at"#,
        )
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            AppError::Conflict("Only draft or validated versions can be validated".to_string())
        })?)
    }

    pub async fn record_simulation(
        &self,
        organization_id: Uuid,
        set_id: Uuid,
        version_id: Uuid,
        hash: &str,
    ) -> Result<(), AppError> {
        self.get_version(organization_id, set_id, version_id)
            .await?;
        sqlx::query(
            r#"UPDATE pricing_rule_versions
               SET "simulationHash" = $2
               WHERE id = $1 AND status IN ('draft', 'validated')"#,
        )
        .bind(version_id)
        .bind(hash)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn publish(
        &self,
        organization_id: Uuid,
        set_id: Uuid,
        version_id: Uuid,
        effective_at: chrono::DateTime<chrono::Utc>,
        actor_id: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        let mut tx = self.pool.begin().await?;
        let owned_set: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT id FROM pricing_rule_sets
               WHERE id = $1 AND "organizationId" = $2
               FOR UPDATE"#,
        )
        .bind(set_id)
        .bind(organization_id)
        .fetch_optional(&mut *tx)
        .await?;
        if owned_set.is_none() {
            return Err(AppError::NotFound(format!(
                "Pricing rule set {set_id} not found"
            )));
        }
        let version = sqlx::query_as::<_, PricingRuleVersion>(
            r#"SELECT v.id, v."ruleSetId" AS rule_set_id, v.version,
                      v.status::text AS status, v.policy,
                      v."simulationHash" AS simulation_hash,
                      v."validatedAt" AS validated_at, v."effectiveAt" AS effective_at,
                      v."publishedAt" AS published_at, v."createdBy" AS created_by,
                      v."publishedBy" AS published_by, v."createdAt" AS created_at
               FROM pricing_rule_versions v
               WHERE v.id = $1 AND v."ruleSetId" = $2
               FOR UPDATE"#,
        )
        .bind(version_id)
        .bind(set_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("Pricing rule version {version_id} not found"))
        })?;
        if version.status != "validated" || version.simulation_hash.is_none() {
            return Err(AppError::Conflict(
                "A pricing version must be validated and simulated before publication".to_string(),
            ));
        }
        let scheduled = effective_at > chrono::Utc::now();
        if !scheduled {
            sqlx::query(
                r#"UPDATE pricing_rule_versions
                   SET status = 'superseded'
                   WHERE "ruleSetId" = $1 AND status = 'published'"#,
            )
            .bind(set_id)
            .execute(&mut *tx)
            .await?;
        }
        let published = sqlx::query_as::<_, PricingRuleVersion>(
            r#"UPDATE pricing_rule_versions
               SET status = $2::pricing_rule_version_status,
                   "effectiveAt" = $3,
                   "publishedAt" = CASE WHEN $2 = 'published' THEN NOW() ELSE NULL END,
                   "publishedBy" = $4
               WHERE id = $1
               RETURNING id, "ruleSetId" AS rule_set_id, version,
                         status::text AS status, policy,
                         "simulationHash" AS simulation_hash,
                         "validatedAt" AS validated_at, "effectiveAt" AS effective_at,
                         "publishedAt" AS published_at, "createdBy" AS created_by,
                         "publishedBy" AS published_by, "createdAt" AS created_at"#,
        )
        .bind(version_id)
        .bind(if scheduled { "scheduled" } else { "published" })
        .bind(effective_at)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await?;
        if !scheduled {
            sqlx::query(
                r#"UPDATE pricing_rule_sets SET "activeVersionId" = $2, "updatedAt" = NOW() WHERE id = $1"#,
            )
            .bind(set_id)
            .bind(version_id)
            .execute(&mut *tx)
            .await?;
        }
        OutboxService::publish_in_tx(
            &mut tx,
            "configuration",
            "pricing.rules.changed",
            serde_json::json!({
                "organizationId": organization_id,
                "ruleSetId": set_id,
                "versionId": published.id,
                "version": published.version,
                "status": published.status,
                "effectiveAt": published.effective_at,
            }),
            Some("admin"),
            None,
            None,
            true,
        )
        .await?;
        tx.commit().await?;
        Ok(published)
    }

    pub async fn activate_due(&self) -> Result<Vec<(Uuid, Uuid, Uuid, i32)>, AppError> {
        let rows: Vec<(Uuid, Uuid, Uuid, i32)> = sqlx::query_as(
            r#"SELECT v.id, v."ruleSetId", s."organizationId", v.version
               FROM pricing_rule_versions v
               JOIN pricing_rule_sets s ON s.id = v."ruleSetId"
               WHERE v.status = 'scheduled' AND v."effectiveAt" <= NOW()
               ORDER BY v."effectiveAt", v.version
               LIMIT 100"#,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut activated = Vec::new();
        for (version_id, set_id, organization_id, version) in rows {
            let mut tx = self.pool.begin().await?;
            sqlx::query(r#"SELECT id FROM pricing_rule_sets WHERE id = $1 FOR UPDATE"#)
                .bind(set_id)
                .execute(&mut *tx)
                .await?;
            let status: Option<(String,)> = sqlx::query_as(
                r#"SELECT status::text FROM pricing_rule_versions
                   WHERE id = $1
                   FOR UPDATE"#,
            )
            .bind(version_id)
            .fetch_optional(&mut *tx)
            .await?;
            if !status.is_some_and(|(value,)| value == "scheduled") {
                tx.rollback().await?;
                continue;
            }
            sqlx::query(
                r#"UPDATE pricing_rule_versions SET status = 'superseded'
                   WHERE "ruleSetId" = $1 AND status = 'published'"#,
            )
            .bind(set_id)
            .execute(&mut *tx)
            .await?;
            let updated = sqlx::query(
                r#"UPDATE pricing_rule_versions
                   SET status = 'published', "publishedAt" = NOW()
                   WHERE id = $1 AND status = 'scheduled'"#,
            )
            .bind(version_id)
            .execute(&mut *tx)
            .await?;
            if updated.rows_affected() == 1 {
                sqlx::query(
                    r#"UPDATE pricing_rule_sets SET "activeVersionId" = $2, "updatedAt" = NOW() WHERE id = $1"#,
                )
                .bind(set_id)
                .bind(version_id)
                .execute(&mut *tx)
                .await?;
                OutboxService::publish_in_tx(
                    &mut tx,
                    "configuration",
                    "pricing.rules.changed",
                    serde_json::json!({
                        "organizationId": organization_id,
                        "ruleSetId": set_id,
                        "versionId": version_id,
                        "version": version,
                        "status": "published",
                        "activatedAt": chrono::Utc::now(),
                    }),
                    Some("admin"),
                    None,
                    None,
                    true,
                )
                .await?;
                activated.push((organization_id, set_id, version_id, version));
            }
            tx.commit().await?;
        }
        Ok(activated)
    }
}

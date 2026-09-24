use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    OrganizationMembershipContext, SettingHistoryQuery, SettingOverride, SettingRevision,
    UpsertSettingOverrideDto, VenueLocation,
};
use crate::realtime::OutboxService;

#[derive(Clone)]
pub struct SettingsRepository {
    pool: PgPool,
}

impl SettingsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn membership_context(
        &self,
        user_id: Uuid,
    ) -> Result<Option<OrganizationMembershipContext>, AppError> {
        Ok(sqlx::query_as::<_, OrganizationMembershipContext>(
            r#"SELECT "organizationId" AS organization_id, role, permissions
               FROM organization_memberships
               WHERE "userId" = $1 AND "isActive" = TRUE
               ORDER BY "createdAt" ASC
               LIMIT 1"#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn ensure_access(
        &self,
        organization_id: Uuid,
        user_id: Uuid,
        permission: &str,
    ) -> Result<(), AppError> {
        let legacy_permission = match permission {
            "settings:read" | "rules:read" => Some("config:read"),
            "settings:write" | "rules:edit" | "rules:publish" => Some("config:write"),
            _ => None,
        };
        let allowed: (bool,) = sqlx::query_as(
            r#"SELECT EXISTS (
                 SELECT 1 FROM organization_memberships
                 WHERE "organizationId" = $1
                   AND "userId" = $2
                   AND "isActive" = TRUE
                   AND (role = 'admin' OR permissions ? $3 OR permissions ? $4)
               )"#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(permission)
        .bind(legacy_permission.unwrap_or("__no_legacy_permission__"))
        .fetch_one(&self.pool)
        .await?;
        if allowed.0 {
            Ok(())
        } else {
            Err(AppError::Forbidden(format!(
                "Missing '{permission}' access for organization {organization_id}"
            )))
        }
    }

    pub async fn ensure_location_access(
        &self,
        organization_id: Uuid,
        location_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        let allowed: (bool,) = sqlx::query_as(
            r#"SELECT EXISTS (
                 SELECT 1
                 FROM venue_locations l
                 JOIN organization_memberships m
                   ON m."organizationId" = l."organizationId"
                  AND m."userId" = $3
                  AND m."isActive" = TRUE
                 LEFT JOIN location_access_assignments a
                   ON a."membershipId" = m.id AND a."locationId" = l.id
                 WHERE l.id = $1
                   AND l."organizationId" = $2
                   AND l."isActive" = TRUE
                   AND (m.role = 'admin' OR a."locationId" IS NOT NULL)
               )"#,
        )
        .bind(location_id)
        .bind(organization_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        if allowed.0 {
            Ok(())
        } else {
            Err(AppError::Forbidden(format!(
                "Missing access to location {location_id} in organization {organization_id}"
            )))
        }
    }

    pub async fn validate_location(
        &self,
        organization_id: Uuid,
        location_id: Uuid,
    ) -> Result<(), AppError> {
        let exists: (bool,) = sqlx::query_as(
            r#"SELECT EXISTS (
                 SELECT 1 FROM venue_locations
                 WHERE id = $1 AND "organizationId" = $2 AND "isActive" = TRUE
               )"#,
        )
        .bind(location_id)
        .bind(organization_id)
        .fetch_one(&self.pool)
        .await?;
        if exists.0 {
            Ok(())
        } else {
            Err(AppError::NotFound(format!(
                "Location {location_id} was not found in organization {organization_id}"
            )))
        }
    }

    pub async fn list_locations(
        &self,
        organization_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<VenueLocation>, AppError> {
        Ok(sqlx::query_as::<_, VenueLocation>(
            r#"SELECT l.id, l."organizationId" AS organization_id, l.slug, l.name,
                      l.timezone, l.currency, l."isActive" AS is_active
               FROM venue_locations l
               JOIN organization_memberships m
                 ON m."organizationId" = l."organizationId"
                AND m."userId" = $2 AND m."isActive" = TRUE
               LEFT JOIN location_access_assignments a
                 ON a."membershipId" = m.id AND a."locationId" = l.id
               WHERE l."organizationId" = $1 AND l."isActive" = TRUE
                 AND (m.role = 'admin' OR a."locationId" IS NOT NULL)
               ORDER BY l.name, l.id"#,
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_overrides(
        &self,
        organization_id: Uuid,
        location_id: Option<Uuid>,
    ) -> Result<Vec<SettingOverride>, AppError> {
        Ok(sqlx::query_as::<_, SettingOverride>(
            r#"SELECT id, "organizationId" AS organization_id,
                      "locationId" AS location_id, key, value, revision,
                      "createdBy" AS created_by, "updatedBy" AS updated_by,
                      "createdAt" AS created_at, "updatedAt" AS updated_at
               FROM setting_overrides
               WHERE "organizationId" = $1
                 AND ("locationId" IS NULL OR "locationId" = $2)
               ORDER BY key, "locationId" NULLS FIRST"#,
        )
        .bind(organization_id)
        .bind(location_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn find_override(
        &self,
        organization_id: Uuid,
        location_id: Option<Uuid>,
        key: &str,
    ) -> Result<Option<SettingOverride>, AppError> {
        Ok(sqlx::query_as::<_, SettingOverride>(
            r#"SELECT id, "organizationId" AS organization_id,
                      "locationId" AS location_id, key, value, revision,
                      "createdBy" AS created_by, "updatedBy" AS updated_by,
                      "createdAt" AS created_at, "updatedAt" AS updated_at
               FROM setting_overrides
               WHERE "organizationId" = $1
                 AND "locationId" IS NOT DISTINCT FROM $2
                 AND key = $3"#,
        )
        .bind(organization_id)
        .bind(location_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn upsert_override(
        &self,
        organization_id: Uuid,
        key: &str,
        dto: &UpsertSettingOverrideDto,
        actor_id: Uuid,
        request_id: Option<&str>,
        mirror_legacy: bool,
    ) -> Result<SettingOverride, AppError> {
        let mut tx = self.pool.begin().await?;
        let scope_lock = format!(
            "{organization_id}:{}:{key}",
            dto.location_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "organization".to_string())
        );
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(scope_lock)
            .execute(&mut *tx)
            .await?;
        let existing = sqlx::query_as::<_, SettingOverride>(
            r#"SELECT id, "organizationId" AS organization_id,
                      "locationId" AS location_id, key, value, revision,
                      "createdBy" AS created_by, "updatedBy" AS updated_by,
                      "createdAt" AS created_at, "updatedAt" AS updated_at
               FROM setting_overrides
               WHERE "organizationId" = $1
                 AND "locationId" IS NOT DISTINCT FROM $2
                 AND key = $3
               FOR UPDATE"#,
        )
        .bind(organization_id)
        .bind(dto.location_id)
        .bind(key)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(expected) = dto.expected_revision {
            let actual = existing.as_ref().map(|row| row.revision).unwrap_or(0);
            if expected != actual {
                return Err(AppError::conflict_code(
                    "SETTING_REVISION_CONFLICT",
                    Some(
                        serde_json::json!({"expectedRevision": expected, "actualRevision": actual}),
                    ),
                ));
            }
        }

        let revision = existing.as_ref().map(|row| row.revision + 1).unwrap_or(1);
        let row = if let Some(existing) = &existing {
            sqlx::query_as::<_, SettingOverride>(
                r#"UPDATE setting_overrides
                   SET value = $2, revision = $3, "updatedBy" = $4, "updatedAt" = NOW()
                   WHERE id = $1
                   RETURNING id, "organizationId" AS organization_id,
                             "locationId" AS location_id, key, value, revision,
                             "createdBy" AS created_by, "updatedBy" AS updated_by,
                             "createdAt" AS created_at, "updatedAt" AS updated_at"#,
            )
            .bind(existing.id)
            .bind(&dto.value)
            .bind(revision)
            .bind(actor_id)
            .fetch_one(&mut *tx)
            .await?
        } else {
            sqlx::query_as::<_, SettingOverride>(
                r#"INSERT INTO setting_overrides
                     ("organizationId", "locationId", key, value, revision, "createdBy", "updatedBy")
                   VALUES ($1, $2, $3, $4, $5, $6, $6)
                   RETURNING id, "organizationId" AS organization_id,
                             "locationId" AS location_id, key, value, revision,
                             "createdBy" AS created_by, "updatedBy" AS updated_by,
                             "createdAt" AS created_at, "updatedAt" AS updated_at"#,
            )
            .bind(organization_id)
            .bind(dto.location_id)
            .bind(key)
            .bind(&dto.value)
            .bind(revision)
            .bind(actor_id)
            .fetch_one(&mut *tx)
            .await?
        };

        sqlx::query(
            r#"INSERT INTO setting_revisions
                 ("organizationId", "locationId", key, revision, operation,
                  "oldValue", "newValue", reason, "actorUserId", "requestId")
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
        )
        .bind(organization_id)
        .bind(dto.location_id)
        .bind(key)
        .bind(revision)
        .bind(if existing.is_some() {
            "update"
        } else {
            "create"
        })
        .bind(existing.as_ref().map(|row| &row.value))
        .bind(&dto.value)
        .bind(dto.reason.trim())
        .bind(actor_id)
        .bind(request_id)
        .execute(&mut *tx)
        .await?;

        OutboxService::publish_in_tx(
            &mut tx,
            "configuration",
            "configuration.changed",
            serde_json::json!({
                "organizationId": organization_id,
                "locationId": dto.location_id,
                "key": key,
                "revision": revision,
            }),
            Some("admin"),
            None,
            None,
            true,
        )
        .await?;
        if mirror_legacy {
            let category = key.split('.').next().unwrap_or("general");
            sqlx::query(
                r#"INSERT INTO configurations
                     (key, value, category, "createdBy", "updatedBy")
                   VALUES ($1, $2, $3, $4, $4)
                   ON CONFLICT (key) DO UPDATE SET
                     value = EXCLUDED.value,
                     category = EXCLUDED.category,
                     "updatedBy" = EXCLUDED."updatedBy",
                     "updatedAt" = NOW()"#,
            )
            .bind(key)
            .bind(&dto.value)
            .bind(category)
            .bind(actor_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(row)
    }

    pub async fn delete_override(
        &self,
        organization_id: Uuid,
        location_id: Option<Uuid>,
        key: &str,
        expected_revision: Option<i64>,
        reason: &str,
        actor_id: Uuid,
        request_id: Option<&str>,
        mirror_legacy_value: Option<&serde_json::Value>,
    ) -> Result<bool, AppError> {
        let mut tx = self.pool.begin().await?;
        let scope_lock = format!(
            "{organization_id}:{}:{key}",
            location_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "organization".to_string())
        );
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(scope_lock)
            .execute(&mut *tx)
            .await?;
        let existing = sqlx::query_as::<_, SettingOverride>(
            r#"SELECT id, "organizationId" AS organization_id,
                      "locationId" AS location_id, key, value, revision,
                      "createdBy" AS created_by, "updatedBy" AS updated_by,
                      "createdAt" AS created_at, "updatedAt" AS updated_at
               FROM setting_overrides
               WHERE "organizationId" = $1
                 AND "locationId" IS NOT DISTINCT FROM $2
                 AND key = $3
               FOR UPDATE"#,
        )
        .bind(organization_id)
        .bind(location_id)
        .bind(key)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(existing) = existing else {
            return Ok(false);
        };
        if let Some(expected) = expected_revision {
            if expected != existing.revision {
                return Err(AppError::conflict_code(
                    "SETTING_REVISION_CONFLICT",
                    Some(
                        serde_json::json!({"expectedRevision": expected, "actualRevision": existing.revision}),
                    ),
                ));
            }
        }
        sqlx::query("DELETE FROM setting_overrides WHERE id = $1")
            .bind(existing.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"INSERT INTO setting_revisions
                 ("organizationId", "locationId", key, revision, operation,
                  "oldValue", reason, "actorUserId", "requestId")
               VALUES ($1, $2, $3, $4, 'delete', $5, $6, $7, $8)"#,
        )
        .bind(organization_id)
        .bind(location_id)
        .bind(key)
        .bind(existing.revision + 1)
        .bind(existing.value)
        .bind(reason.trim())
        .bind(actor_id)
        .bind(request_id)
        .execute(&mut *tx)
        .await?;
        OutboxService::publish_in_tx(
            &mut tx,
            "configuration",
            "configuration.changed",
            serde_json::json!({
                "organizationId": organization_id,
                "locationId": location_id,
                "key": key,
                "revision": existing.revision + 1,
                "deleted": true,
            }),
            Some("admin"),
            None,
            None,
            true,
        )
        .await?;
        if let Some(inherited_value) = mirror_legacy_value {
            let category = key.split('.').next().unwrap_or("general");
            sqlx::query(
                r#"INSERT INTO configurations
                     (key, value, category, "createdBy", "updatedBy")
                   VALUES ($1, $2, $3, $4, $4)
                   ON CONFLICT (key) DO UPDATE SET
                     value = EXCLUDED.value,
                     category = EXCLUDED.category,
                     "updatedBy" = EXCLUDED."updatedBy",
                     "updatedAt" = NOW()"#,
            )
            .bind(key)
            .bind(inherited_value)
            .bind(category)
            .bind(actor_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(true)
    }

    pub async fn history(
        &self,
        organization_id: Uuid,
        query: &SettingHistoryQuery,
    ) -> Result<Vec<SettingRevision>, AppError> {
        let limit = query.limit.unwrap_or(100).clamp(1, 500);
        Ok(sqlx::query_as::<_, SettingRevision>(
            r#"SELECT id, "organizationId" AS organization_id,
                      "locationId" AS location_id, key, revision, operation,
                      "oldValue" AS old_value, "newValue" AS new_value,
                      reason, "actorUserId" AS actor_user_id,
                      "requestId" AS request_id, "createdAt" AS created_at
               FROM setting_revisions
               WHERE "organizationId" = $1
                 AND "locationId" IS NOT DISTINCT FROM $2
                 AND ($3::text IS NULL OR key = $3)
               ORDER BY "createdAt" DESC, id DESC
               LIMIT $4"#,
        )
        .bind(organization_id)
        .bind(query.location_id)
        .bind(query.key.as_deref())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn latest_revision(&self, organization_id: Uuid) -> Result<i64, AppError> {
        let row: (i64,) = sqlx::query_as(
            r#"SELECT COALESCE(MAX(id), 0) FROM setting_revisions WHERE "organizationId" = $1"#,
        )
        .bind(organization_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }
}

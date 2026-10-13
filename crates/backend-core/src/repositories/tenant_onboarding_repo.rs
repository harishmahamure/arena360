//! Setup progress and live readiness share the tenant's fenced writer and revision boundary.
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::SqliteConnection;
use uuid::Uuid;

use crate::{error::AppError, models::onboarding::*, tenancy::TenantDb};

const PROGRESS: &str = "SELECT current_step,revision,started_at,settings_reviewed_at,completed_at,updated_at FROM tenant_onboarding WHERE singleton=1";

pub struct TenantOnboardingRepository {
    db: Arc<TenantDb>,
}

impl TenantOnboardingRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }

    pub async fn snapshot(&self, actor: Uuid) -> Result<OnboardingSnapshot, AppError> {
        let pool = self.db.read_pool()?;
        let mut tx = pool.begin().await?;
        authorize(&mut tx, actor, "settings:read").await?;
        let snapshot = snapshot_on(&mut tx).await?;
        tx.commit().await?;
        Ok(snapshot)
    }

    pub async fn save(
        &self,
        actor: Uuid,
        dto: SaveOnboardingDto,
    ) -> Result<OnboardingSnapshot, AppError> {
        let tenant = self.db.tenant_id();
        self.db.with_immediate_writer(move |c| Box::pin(async move {
            authorize(c, actor, "settings:write").await?;
            let progress: OnboardingProgress = sqlx::query_as(PROGRESS).fetch_one(&mut *c).await?;
            check_revision(&progress, dto.expected_revision)?;
            if progress.completed_at.is_some() {
                return Err(AppError::Conflict("Setup is already complete".into()));
            }
            let at = super::tenant_back_office::now()?;
            sqlx::query("UPDATE tenant_onboarding SET current_step=?,revision=revision+1,started_at=COALESCE(started_at,?),settings_reviewed_at=CASE WHEN ? THEN ? ELSE settings_reviewed_at END,updated_at=?,updated_by=? WHERE singleton=1")
                .bind(dto.current_step).bind(&at).bind(dto.review_settings).bind(&at).bind(&at).bind(actor.to_string()).execute(&mut *c).await?;
            let snapshot = snapshot_on(c).await?;
            super::tenant_back_office::event(c, "onboarding", tenant, "onboarding.progress_saved", None, false, json!({"progress":snapshot.progress,"actorId":actor})).await?;
            Ok(snapshot)
        })).await
    }

    pub async fn complete(
        &self,
        actor: Uuid,
        dto: CompleteOnboardingDto,
    ) -> Result<OnboardingSnapshot, AppError> {
        let tenant = self.db.tenant_id();
        self.db.with_immediate_writer(move |c| Box::pin(async move {
            authorize(c, actor, "settings:write").await?;
            let snapshot = snapshot_on(c).await?;
            check_revision(&snapshot.progress, dto.expected_revision)?;
            if snapshot.progress.completed_at.is_some() { return Ok(snapshot); }
            if !snapshot.readiness.ready {
                return Err(AppError::Api { code: "ONBOARDING_NOT_READY".into(), status: StatusCode::CONFLICT, details: Some(json!(snapshot.readiness)) });
            }
            let at = super::tenant_back_office::now()?;
            sqlx::query("UPDATE tenant_onboarding SET current_step='review',revision=revision+1,started_at=COALESCE(started_at,?),completed_at=?,updated_at=?,updated_by=? WHERE singleton=1")
                .bind(&at).bind(&at).bind(&at).bind(actor.to_string()).execute(&mut *c).await?;
            let snapshot = snapshot_on(c).await?;
            super::tenant_back_office::event(c, "onboarding", tenant, "onboarding.completed", None, false, json!({"progress":snapshot.progress,"actorId":actor})).await?;
            Ok(snapshot)
        })).await
    }
}

async fn authorize(
    c: &mut SqliteConnection,
    actor: Uuid,
    permission: &str,
) -> Result<(), AppError> {
    let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users u WHERE u.id=? AND u.role='admin' AND u.is_active=1 AND u.deleted_at IS NULL AND EXISTS(SELECT 1 FROM access_assignments a JOIN access_roles r ON r.id=a.role_id JOIN json_each(r.permissions) p ON p.value=? WHERE a.user_id=u.id AND r.is_template=0) AND COALESCE((SELECT enabled FROM access_modules WHERE module='settings'),1)=1)")
        .bind(actor.to_string()).bind(permission).fetch_one(c).await?;
    if allowed {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!(
            "Organization administrator with {permission} required"
        )))
    }
}

fn check_revision(progress: &OnboardingProgress, expected: i64) -> Result<(), AppError> {
    if expected < 0 {
        return Err(AppError::BadRequest(
            "Expected revision must be nonnegative".into(),
        ));
    }
    if progress.revision == expected {
        Ok(())
    } else {
        Err(AppError::Api {
            code: "ONBOARDING_REVISION_CONFLICT".into(),
            status: StatusCode::CONFLICT,
            details: Some(json!({"currentRevision":progress.revision})),
        })
    }
}

async fn snapshot_on(c: &mut SqliteConnection) -> Result<OnboardingSnapshot, AppError> {
    let progress: OnboardingProgress = sqlx::query_as(PROGRESS).fetch_one(&mut *c).await?;
    // Exact type/subtype matches the wallet/session access contract. Unregistered devices
    // remain usable from the panel; kiosk pairing is an optional follow-up.
    let locations: Vec<OnboardingLocation> = sqlx::query_as(r#"
        SELECT l.id,l.name,l.timezone,l.currency,
          (SELECT count(*) FROM devices d WHERE d.location_id=l.id AND d.deleted_at IS NULL AND d.status IN ('available','operational','in_use')) station_count,
          (SELECT count(*) FROM plans p WHERE p.is_active=1 AND p.deleted_at IS NULL AND p.time_credits>0
            AND (p.availability_scope='ALL' OR EXISTS(SELECT 1 FROM plan_locations pl WHERE pl.plan_id=p.id AND pl.location_id=l.id))
            AND EXISTS(SELECT 1 FROM devices d WHERE d.location_id=l.id AND d.deleted_at IS NULL AND d.status IN ('available','operational','in_use') AND d.device_type=p.device_type AND d.device_sub_type=p.device_sub_type)) compatible_plan_count,
          EXISTS(SELECT 1 FROM users u WHERE u.role IN ('admin','staff') AND u.is_active=1 AND u.deleted_at IS NULL
            AND NOT EXISTS(SELECT 1 FROM json_each('["locations:read","devices:read","plans:read","shifts:read","shifts:write","cash-registers:read","cash-registers:write","players:read","players:write","transactions:read","transactions:write","player-plans:read","player-plans:write","sessions:read","sessions:write"]') needed
              WHERE COALESCE((SELECT enabled FROM access_modules WHERE module=substr(needed.value,1,instr(needed.value,':')-1)),1)=0
                OR NOT EXISTS(SELECT 1 FROM access_assignments a JOIN access_roles r ON r.id=a.role_id JOIN json_each(r.permissions) p ON p.value=needed.value WHERE a.user_id=u.id AND r.is_template=0)
                OR (u.role='staff' AND NOT EXISTS(SELECT 1 FROM location_role_assignments a JOIN access_roles r ON r.id=a.role_id JOIN json_each(r.permissions) p ON p.value=needed.value WHERE a.user_id=u.id AND a.location_id=l.id AND r.is_template=0)))) operator_ready
        FROM venue_locations l WHERE l.is_active=1 ORDER BY l.name,l.id
    "#).fetch_all(&mut *c).await?;
    let valid_venue = |l: &OnboardingLocation| {
        l.timezone.parse::<chrono_tz::Tz>().is_ok()
            && crate::services::settings_catalog::validate(
                "UTC",
                "pricing.currency",
                &json!(l.currency),
                true,
            )
            .is_ok()
    };
    let venue = locations.iter().any(&valid_venue);
    let stations = locations
        .iter()
        .any(|l| valid_venue(l) && l.station_count > 0);
    let pricing = locations
        .iter()
        .any(|l| valid_venue(l) && l.compatible_plan_count > 0);
    let operator = locations
        .iter()
        .any(|l| valid_venue(l) && l.compatible_plan_count > 0 && l.operator_ready);
    let checks = [
        (
            "venue",
            "Active venue with a valid timezone and currency",
            venue,
            "/locations",
        ),
        (
            "settings",
            "Operating settings reviewed",
            progress.settings_reviewed_at.is_some(),
            "/onboarding",
        ),
        (
            "stations",
            "At least one usable station",
            stations,
            "/devices",
        ),
        (
            "pricing",
            "Active plan with time credit matching a station and its venue",
            pricing,
            "/plans",
        ),
        (
            "operator",
            "An operator with counter and session access at that venue",
            operator,
            "/access?tab=team",
        ),
    ]
    .into_iter()
    .map(|(key, label, ready, path)| OnboardingCheck {
        key: key.into(),
        label: label.into(),
        ready,
        action_path: path.into(),
    })
    .collect::<Vec<_>>();
    let ready = checks.iter().all(|check| check.ready);
    Ok(OnboardingSnapshot {
        progress,
        readiness: OnboardingReadiness {
            ready,
            checks,
            locations,
        },
    })
}

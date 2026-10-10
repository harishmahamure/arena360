//! Tenant settings precedence, revision conflicts and permission boundaries.
mod support;
use gaming_cafe_api::{
    cache::NoopCache,
    error::AppError,
    models::{EffectiveSettingsQuery, UpsertSettingOverrideDto},
    repositories::TenantSettingsRepository,
    services::ConfigService,
};
use serde_json::json;
use std::sync::Arc;
use support::TenantFixture;
use uuid::Uuid;
fn dto(location: Option<Uuid>, value: serde_json::Value) -> UpsertSettingOverrideDto {
    UpsertSettingOverrideDto {
        location_id: location,
        value,
        reason: "configuration acceptance test".into(),
        expected_revision: Some(0),
    }
}
async fn venue(f: &TenantFixture) -> Uuid {
    TenantSettingsRepository::new(f.db.clone())
        .save_location(
            f.db.tenant_id(),
            None,
            serde_json::from_value(
                json!({"slug":"main","name":"Main","timezone":"UTC","currency":"INR"}),
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .id
}
#[tokio::test]
async fn location_override_precedes_organization_and_records_audit_event() {
    let f = TenantFixture::new().await;
    let org = f.db.tenant_id();
    let location = venue(&f).await;
    let actor = f.player("settings-actor").await;
    let service = ConfigService::new(Arc::new(NoopCache), "UTC".into());
    let baseline = service
        .snapshot_all_tenant(f.db.clone(), org, Some(location))
        .await
        .unwrap();
    for (scope, value) in [
        (None, "Organization venue"),
        (Some(location), "Location venue"),
    ] {
        service
            .upsert_setting_tenant(
                f.db.clone(),
                org,
                "business.name",
                dto(scope, json!(value)),
                actor,
                Some("settings-acceptance"),
            )
            .await
            .unwrap();
    }
    let effective = service
        .effective_tenant(
            f.db.clone(),
            org,
            EffectiveSettingsQuery {
                location_id: Some(location),
                category: Some("business".into()),
            },
        )
        .await
        .unwrap();
    let name = effective.iter().find(|s| s.key == "business.name").unwrap();
    assert_eq!(name.value, json!("Location venue"));
    assert_eq!(name.source_scope, "location");
    let snapshot = service
        .snapshot_all_tenant(f.db.clone(), org, Some(location))
        .await
        .unwrap();
    assert!(snapshot.revision > baseline.revision);
    assert_ne!(snapshot.etag, baseline.etag);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM setting_revisions WHERE key='business.name'"
        )
        .fetch_one(&f.db.read_pool().unwrap())
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM outbox_events WHERE event_type='configuration.changed'"
        )
        .fetch_one(&f.db.read_pool().unwrap())
        .await
        .unwrap(),
        2
    );
    f.close().await;
}
#[tokio::test]
async fn concurrent_create_returns_revision_conflict_without_lost_update() {
    let f = TenantFixture::new().await;
    let org = f.db.tenant_id();
    let actor = f.player("settings-actor").await;
    let service = ConfigService::new(Arc::new(NoopCache), "UTC".into());
    let (left, right) = tokio::join!(
        service.upsert_setting_tenant(
            f.db.clone(),
            org,
            "business.phone",
            dto(None, json!("1111111111")),
            actor,
            None
        ),
        service.upsert_setting_tenant(
            f.db.clone(),
            org,
            "business.phone",
            dto(None, json!("2222222222")),
            actor,
            None
        )
    );
    assert_eq!(left.is_ok() as u8 + right.is_ok() as u8, 1);
    assert!(
        matches!(left.err().or_else(||right.err()).unwrap(),AppError::Api {ref code,..} if code=="SETTING_REVISION_CONFLICT")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM setting_overrides WHERE key='business.phone'"
        )
        .fetch_one(&f.db.read_pool().unwrap())
        .await
        .unwrap(),
        1
    );
    f.close().await;
}
#[tokio::test]
async fn membership_does_not_grant_cross_tenant_or_unassigned_location_access() {
    let f = TenantFixture::new().await;
    let org = f.db.tenant_id();
    let location = venue(&f).await;
    let actor = Uuid::now_v7();
    let role = Uuid::now_v7();
    let repo = TenantSettingsRepository::new(f.db.clone());
    assert!(repo
        .ensure_access(org, actor, "settings:read")
        .await
        .is_err());
    f.db.with_immediate_writer(move|c|Box::pin(async move {
        let at=gaming_cafe_api::time::format_sqlite_timestamp(&chrono::Utc::now()).unwrap();
        sqlx::query("INSERT INTO users(id,username,role,created_at,updated_at) VALUES(?,'settings-staff','staff',?,?)").bind(actor.to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
        sqlx::query("INSERT INTO access_roles(id,name,permissions,created_at,updated_at) VALUES(?,'Settings reader','[\"settings:read\"]',?,?)").bind(role.to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
        sqlx::query("INSERT INTO access_assignments(user_id,role_id,created_at) VALUES(?,?,?)").bind(actor.to_string()).bind(role.to_string()).bind(&at).execute(c).await?;Ok(())
    })).await.unwrap();
    repo.ensure_access(org, actor, "settings:read")
        .await
        .unwrap();
    assert!(repo
        .ensure_access(Uuid::now_v7(), actor, "settings:read")
        .await
        .is_err());
    assert!(repo
        .ensure_location_access(org, location, actor)
        .await
        .is_err());
    f.close().await;
}

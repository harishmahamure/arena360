//! Database acceptance tests for scoped settings isolation and concurrency.
//! Run with: `cargo test --test configuration_policy -- --ignored`

use std::sync::Arc;

use gaming_cafe_api::cache::NoopCache;
use gaming_cafe_api::config::load_dotenv;
use gaming_cafe_api::error::AppError;
use gaming_cafe_api::models::{EffectiveSettingsQuery, UpsertSettingOverrideDto};
use gaming_cafe_api::repositories::SettingsRepository;
use gaming_cafe_api::services::ConfigService;
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

async fn pool() -> Option<PgPool> {
    load_dotenv();
    let database_url = std::env::var("DATABASE_URL").ok()?;
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .ok()
}

async fn actor_id(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(r#"SELECT id FROM users ORDER BY "createdAt" LIMIT 1"#)
        .fetch_one(pool)
        .await
        .expect("seeded test user")
}

async fn create_scope(pool: &PgPool) -> (Uuid, Uuid) {
    let organization_id = Uuid::new_v4();
    let location_id = Uuid::new_v4();
    let slug = format!("test-{}", &organization_id.simple().to_string()[..12]);
    sqlx::query("INSERT INTO organizations (id, slug, name) VALUES ($1, $2, 'Test organization')")
        .bind(organization_id)
        .bind(&slug)
        .execute(pool)
        .await
        .expect("organization");
    sqlx::query(
        r#"INSERT INTO venue_locations (id, "organizationId", slug, name)
           VALUES ($1, $2, 'main', 'Test location')"#,
    )
    .bind(location_id)
    .bind(organization_id)
    .execute(pool)
    .await
    .expect("location");
    (organization_id, location_id)
}

async fn cleanup(pool: &PgPool, organization_id: Uuid) {
    let _ = sqlx::query("DELETE FROM realtime_outbox WHERE payload->>'organizationId' = $1")
        .bind(organization_id.to_string())
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM organizations WHERE id = $1")
        .bind(organization_id)
        .execute(pool)
        .await;
}

fn override_dto(location_id: Option<Uuid>, value: serde_json::Value) -> UpsertSettingOverrideDto {
    UpsertSettingOverrideDto {
        location_id,
        value,
        reason: "configuration policy acceptance test".to_string(),
        expected_revision: Some(0),
    }
}

#[tokio::test]
#[ignore = "requires DATABASE_URL with configuration migration applied"]
async fn location_override_precedes_organization_and_records_audit_event() {
    let Some(pool) = pool().await else { return };
    let actor_id = actor_id(&pool).await;
    let (organization_id, location_id) = create_scope(&pool).await;
    let repo = SettingsRepository::new(pool.clone());

    repo.upsert_override(
        organization_id,
        "business.name",
        &override_dto(None, json!("Organization venue")),
        actor_id,
        Some("settings-acceptance-org"),
        false,
    )
    .await
    .expect("organization override");
    repo.upsert_override(
        organization_id,
        "business.name",
        &override_dto(Some(location_id), json!("Location venue")),
        actor_id,
        Some("settings-acceptance-location"),
        false,
    )
    .await
    .expect("location override");

    let service = ConfigService::new(pool.clone(), Arc::new(NoopCache), "UTC".to_string());
    let effective = service
        .effective(
            organization_id,
            EffectiveSettingsQuery {
                location_id: Some(location_id),
                category: Some("business".to_string()),
            },
        )
        .await
        .expect("effective settings");
    let business_name = effective
        .iter()
        .find(|setting| setting.key == "business.name")
        .expect("business.name");
    assert_eq!(business_name.value, json!("Location venue"));
    assert_eq!(business_name.source_scope, "location");

    let revision_count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM setting_revisions
           WHERE "organizationId" = $1 AND key = 'business.name'"#,
    )
    .bind(organization_id)
    .fetch_one(&pool)
    .await
    .expect("revisions");
    let event_count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM realtime_outbox
           WHERE event_type = 'configuration.changed'
             AND payload->>'organizationId' = $1"#,
    )
    .bind(organization_id.to_string())
    .fetch_one(&pool)
    .await
    .expect("outbox events");
    assert_eq!(revision_count, 2);
    assert_eq!(event_count, 2);

    cleanup(&pool, organization_id).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL with configuration migration applied"]
async fn concurrent_create_returns_revision_conflict_without_lost_update() {
    let Some(pool) = pool().await else { return };
    let actor_id = actor_id(&pool).await;
    let (organization_id, _) = create_scope(&pool).await;
    let first = SettingsRepository::new(pool.clone());
    let second = SettingsRepository::new(pool.clone());
    let first_dto = override_dto(None, json!("1111111111"));
    let second_dto = override_dto(None, json!("2222222222"));

    let (left, right) = tokio::join!(
        first.upsert_override(
            organization_id,
            "business.phone",
            &first_dto,
            actor_id,
            Some("concurrency-left"),
            false,
        ),
        second.upsert_override(
            organization_id,
            "business.phone",
            &second_dto,
            actor_id,
            Some("concurrency-right"),
            false,
        ),
    );

    assert_eq!(left.is_ok() as u8 + right.is_ok() as u8, 1);
    let conflict = left.err().or_else(|| right.err()).expect("one conflict");
    assert!(matches!(
        conflict,
        AppError::Api { ref code, .. } if code == "SETTING_REVISION_CONFLICT"
    ));
    let rows: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM setting_overrides
           WHERE "organizationId" = $1 AND key = 'business.phone'"#,
    )
    .bind(organization_id)
    .fetch_one(&pool)
    .await
    .expect("override count");
    assert_eq!(rows, 1);

    cleanup(&pool, organization_id).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL with configuration migration applied"]
async fn membership_does_not_grant_cross_organization_or_unassigned_location_access() {
    let Some(pool) = pool().await else { return };
    let actor_id = actor_id(&pool).await;
    let (organization_id, location_id) = create_scope(&pool).await;
    let repo = SettingsRepository::new(pool.clone());

    assert!(matches!(
        repo.ensure_access(organization_id, actor_id, "settings:read")
            .await,
        Err(AppError::Forbidden(_))
    ));
    sqlx::query(
        r#"INSERT INTO organization_memberships
             ("organizationId", "userId", role, permissions)
           VALUES ($1, $2, 'staff', '["settings:read"]')"#,
    )
    .bind(organization_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .expect("membership");
    repo.ensure_access(organization_id, actor_id, "settings:read")
        .await
        .expect("organization access");
    assert!(matches!(
        repo.ensure_location_access(organization_id, location_id, actor_id)
            .await,
        Err(AppError::Forbidden(_))
    ));

    cleanup(&pool, organization_id).await;
}

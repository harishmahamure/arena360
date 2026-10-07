//! Timed ownership/fencing integration test.
//! CONTROL_TEST_DATABASE_URL=... cargo test --test ownership_fencing -- --ignored

use std::time::Duration as StdDuration;

use chrono::{Duration, Utc};
use gaming_cafe_api::control::{CreateTenant, LeaseClient, LeaseConfig, Repository};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires an isolated control-plane database"]
async fn only_the_current_generation_writes_across_cutoff_and_reassignment() {
    let database_url =
        std::env::var("CONTROL_TEST_DATABASE_URL").expect("CONTROL_TEST_DATABASE_URL is required");
    let control_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&control_pool)
        .await
        .unwrap();

    let suffix = Uuid::new_v4().simple().to_string();
    let first_cell = create_cell(&control_pool, &format!("race-a-{suffix}")).await;
    let second_cell = create_cell(&control_pool, &format!("race-b-{suffix}")).await;
    let tenant = Repository::new(control_pool.clone())
        .create_tenant(CreateTenant {
            slug: format!("race-{suffix}"),
            name: "Ownership race".into(),
            timezone: "UTC".into(),
            owner_cell: Some(first_cell),
            subscription_plan: "trial".into(),
            entitlements: json!({}),
            trial_ends_at: Utc::now() + Duration::days(30),
            entitlement_grace_until: Utc::now() + Duration::days(37),
        })
        .await
        .unwrap();
    let timings = LeaseConfig {
        lease_duration: StdDuration::from_millis(2_000),
        renewal_interval: StdDuration::from_millis(200),
        fence_before_expiry: StdDuration::from_millis(500),
        reassignment_skew: StdDuration::from_millis(500),
    };

    let race_first = LeaseClient::new(control_pool.clone(), first_cell, timings).unwrap();
    let race_second = LeaseClient::new(control_pool.clone(), second_cell, timings).unwrap();
    let (first_attempt, second_attempt) = tokio::join!(
        race_first.acquire(tenant.id),
        race_second.acquire(tenant.id)
    );
    assert_ne!(first_attempt.is_ok(), second_attempt.is_ok());
    let (winner, loser, blocked) = if first_attempt.is_ok() {
        (race_first, race_second, second_attempt)
    } else {
        (race_second, race_first, first_attempt)
    };
    assert!(blocked.is_err());
    assert_eq!(winner.writable_generation(tenant.id).unwrap(), 1);
    assert!(loser.writable_generation(tenant.id).is_err());

    let cutoff_tenant = Repository::new(control_pool.clone())
        .create_tenant(CreateTenant {
            slug: format!("cutoff-{suffix}"),
            name: "Control cutoff".into(),
            timezone: "UTC".into(),
            owner_cell: Some(first_cell),
            subscription_plan: "trial".into(),
            entitlements: json!({}),
            trial_ends_at: Utc::now() + Duration::days(30),
            entitlement_grace_until: Utc::now() + Duration::days(37),
        })
        .await
        .unwrap();
    let first_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .unwrap();
    let cutoff_owner = LeaseClient::new(first_pool.clone(), first_cell, timings).unwrap();
    let replacement = LeaseClient::new(control_pool.clone(), second_cell, timings).unwrap();
    let generation = cutoff_owner
        .acquire(cutoff_tenant.id)
        .await
        .unwrap()
        .ownership_generation;

    first_pool.close().await;
    assert!(cutoff_owner.renew(cutoff_tenant.id).await.is_err());
    tokio::time::sleep(StdDuration::from_millis(750)).await;
    assert_eq!(
        cutoff_owner.writable_generation(cutoff_tenant.id).unwrap(),
        generation
    );
    tokio::time::sleep(StdDuration::from_millis(900)).await;
    assert!(cutoff_owner.writable_generation(cutoff_tenant.id).is_err());

    // PostgreSQL still refuses reassignment until database expiry plus skew.
    assert!(replacement.acquire(cutoff_tenant.id).await.is_err());
    tokio::time::sleep(StdDuration::from_millis(1_100)).await;
    let replacement_grant = replacement.acquire(cutoff_tenant.id).await.unwrap();
    assert_eq!(replacement_grant.ownership_generation, generation + 1);
    replacement
        .ensure_writable(cutoff_tenant.id, generation + 1)
        .unwrap();
    assert!(replacement
        .ensure_writable(cutoff_tenant.id, generation)
        .is_err());
    assert!(cutoff_owner.writable_generation(cutoff_tenant.id).is_err());
}

async fn create_cell(pool: &sqlx::PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO cells (name, address) VALUES ($1, $2) RETURNING id")
        .bind(name)
        .bind(format!("http://{name}.internal"))
        .fetch_one(pool)
        .await
        .unwrap()
}

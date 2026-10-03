//! Uses connection-local temporary tables; leaves all permanent records untouched.
use chrono::{Duration, TimeZone, Utc};
use gaming_cafe_api::config::load_dotenv;
use gaming_cafe_api::models::{
    deduction_profile::DeductionProfile, CreateSessionDto, PricingTarget, DEFAULT_ORGANIZATION_ID,
    DEFAULT_VENUE_LOCATION_ID,
};
use gaming_cafe_api::repositories::SessionRepository;
use gaming_cafe_api::services::{
    deduction_profile::weighted_minutes_between, PricingPolicyService,
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires PostgreSQL; fixtures are temporary tables"]
async fn published_choices_price_plans_and_persist_session_speed_independently() {
    load_dotenv();
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL"))
        .await
        .unwrap();
    for table in [
        "pricing_rule_sets",
        "pricing_rule_versions",
        "usage_sessions",
    ] {
        sqlx::query(&format!(
            "CREATE TEMP TABLE {table} (LIKE public.{table} INCLUDING ALL)"
        ))
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::raw_sql(include_str!(
        "../migrations/20261003090000_session_deduction_policy_snapshot.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let policy = json!({"baseRate": "60", "roundingScale": 2, "maximumPrice": "230", "rules": [
        {"id":"price", "name":"Plan price", "priority":100, "deviceTypes":["PC","PS5"], "weekdays":[], "action":{"type":"multiplier","value":"1.25"}},
        {"id":"speed", "name":"Credit speed", "target":"deduction", "priority":100, "deviceTypes":[], "weekdays":[], "action":{"type":"multiplier","value":"1.25"}}
    ]});
    for (org, location, status) in [
        (
            DEFAULT_ORGANIZATION_ID,
            Some(DEFAULT_VENUE_LOCATION_ID),
            "published",
        ),
        (DEFAULT_ORGANIZATION_ID, None, "draft"),
        (Uuid::new_v4(), None, "published"),
        (DEFAULT_ORGANIZATION_ID, Some(Uuid::new_v4()), "published"),
    ] {
        let set_id = Uuid::new_v4();
        let version_id = Uuid::new_v4();
        sqlx::query(r#"INSERT INTO pricing_rule_sets(id,"organizationId","locationId",name,"activeVersionId") VALUES($1,$2,$3,'Fixture',$4)"#)
            .bind(set_id).bind(org).bind(location).bind(version_id).execute(&pool).await.unwrap();
        sqlx::query(r#"INSERT INTO pricing_rule_versions(id,"ruleSetId",version,status,policy) VALUES($1,$2,1,$3::pricing_rule_version_status,$4)"#)
            .bind(version_id).bind(set_id).bind(status).bind(&policy).execute(&pool).await.unwrap();
    }
    let pricing = PricingPolicyService::new(pool.clone());
    let live = pricing.active_plan_policy().await.unwrap();
    assert_eq!(
        live.rules.len(),
        1,
        "draft and unrelated scopes cannot affect a purchase"
    );
    let at = Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap();
    assert_eq!(
        PricingPolicyService::evaluate_plan_price(200.0, Some("PS5"), &live, at, "Asia/Kolkata")
            .unwrap(),
        230.0
    );
    assert_eq!(
        PricingPolicyService::evaluate_plan_price(200.0, Some("OTHER"), &live, at, "Asia/Kolkata")
            .unwrap(),
        200.0
    );
    let mut profile = DeductionProfile::normal();
    profile.policy_rules = pricing
        .active_rules(
            DEFAULT_ORGANIZATION_ID,
            Some(DEFAULT_VENUE_LOCATION_ID),
            PricingTarget::Deduction,
        )
        .await
        .unwrap();
    assert_eq!(profile.policy_rules.len(), 1);
    let snapshot = serde_json::to_value(&profile).unwrap();
    let sessions = SessionRepository::new(pool.clone());
    let created = sessions
        .create(
            &CreateSessionDto {
                balance_id: Uuid::new_v4(),
                device_id: Uuid::new_v4(),
                shift_id: None,
                start_time: Some(at),
            },
            at,
            None,
            120,
            None,
            &snapshot,
        )
        .await
        .unwrap();
    // Editing the published policy must not recalculate an already-started session.
    sqlx::query("UPDATE pricing_rule_versions SET policy = jsonb_set(policy, '{rules,1,action,value}', '\"5\"')").execute(&pool).await.unwrap();
    let stored = sessions.find_by_id(created.id).await.unwrap().unwrap();
    assert_eq!(stored.deduction_profile_snapshot.as_ref(), Some(&snapshot));
    let preserved: DeductionProfile =
        serde_json::from_value(stored.deduction_profile_snapshot.unwrap()).unwrap();
    let consumed =
        weighted_minutes_between(at, at + Duration::minutes(60), &preserved, "Asia/Kolkata");
    assert_eq!(consumed, 75.0);
    let ended = sessions
        .end(
            created.id,
            at + Duration::minutes(60),
            60,
            Some(consumed as i32),
            None,
        )
        .await
        .unwrap();
    assert_eq!(ended.time_credits_consumed, Some(75));
    assert_eq!(ended.deduction_profile_snapshot, Some(snapshot));
}

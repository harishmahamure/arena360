//! Published plan prices and captured session deduction rules remain independent.
mod support;
use chrono::{Duration, Timelike, Utc};
use gaming_cafe_api::{
    cache::NoopCache,
    models::PricingTarget,
    services::{ConfigService, PlanService, PricingPolicyService},
};
use serde_json::json;
use std::sync::Arc;
use support::SessionFixture;

#[tokio::test]
async fn published_choices_price_plans_and_persist_session_speed_independently() {
    let f = SessionFixture::new().await;
    let db = f.tenant.db.clone();
    let org = db.tenant_id();
    let pricing = PricingPolicyService::new();
    let policy = json!({"baseRate":"60","roundingScale":2,"maximumPrice":"230","rules":[
        {"id":"price","name":"Plan price","priority":100,"deviceTypes":["PC","PS5"],"weekdays":[],"action":{"type":"multiplier","value":"1.25"}},
        {"id":"speed","name":"Credit speed","target":"deduction","priority":100,"deviceTypes":[],"weekdays":[],"action":{"type":"multiplier","value":"1.25"}}
    ]});
    let (set, version) = pricing
        .create_tenant(
            db.clone(),
            org,
            serde_json::from_value(
                json!({"name":"Published choices","locationIds":[f.venue],"policy":policy}),
            )
            .unwrap(),
            f.player,
        )
        .await
        .unwrap();
    // A draft with different rules cannot affect a purchase or login.
    pricing.create_tenant(db.clone(),org,serde_json::from_value(json!({"name":"Unpublished choices","policy":{"baseRate":"60","rules":[],"maximumPrice":"1"}})).unwrap(),f.player).await.unwrap();
    pricing
        .validate_tenant(db.clone(), org, set.id, version.id)
        .await
        .unwrap();
    pricing
        .simulate_tenant(
            db.clone(),
            org,
            set.id,
            version.id,
            serde_json::from_value(json!({"locationId":f.venue,"at":Utc::now(),"deviceType":"PC"}))
                .unwrap(),
            "UTC",
            "INR",
        )
        .await
        .unwrap();
    pricing
        .publish_tenant(
            db.clone(),
            org,
            set.id,
            version.id,
            serde_json::from_value(json!({})).unwrap(),
            f.player,
        )
        .await
        .unwrap();
    let plans = PlanService::new(Arc::new(ConfigService::new(
        Arc::new(NoopCache),
        "UTC".into(),
    )));
    plans
        .update_tenant(
            db.clone(),
            f.plan,
            serde_json::from_value(json!({"price":200})).unwrap(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        plans
            .get_tenant(db.clone(), f.plan, Some(f.venue))
            .await
            .unwrap()
            .current_price,
        Some(230.0)
    );
    assert_eq!(
        pricing
            .active_rules_tenant(db.clone(), org, Some(f.venue), PricingTarget::Deduction)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(pricing
        .active_rules_tenant(
            db.clone(),
            uuid::Uuid::now_v7(),
            Some(f.venue),
            PricingTarget::Deduction
        )
        .await
        .is_err());
    let at = Utc::now().with_nanosecond(0).unwrap() - Duration::minutes(60);
    let session = f.start(Some(at)).await;
    let snapshot = session.deduction_profile_snapshot.clone().unwrap();
    let profile: gaming_cafe_api::models::deduction_profile::DeductionProfile =
        serde_json::from_value(snapshot.clone()).unwrap();
    assert_eq!(
        gaming_cafe_api::services::deduction_profile::weighted_minutes_between(
            at,
            at + Duration::minutes(60),
            &profile,
            "UTC"
        ),
        75.0
    );
    let mut changed = policy.clone();
    changed["rules"][1]["action"]["value"] = json!("5");
    let version2 = pricing
        .create_version_tenant(
            db.clone(),
            org,
            set.id,
            serde_json::from_value(json!({"policy":changed})).unwrap(),
            f.player,
        )
        .await
        .unwrap();
    pricing
        .validate_tenant(db.clone(), org, set.id, version2.id)
        .await
        .unwrap();
    pricing
        .simulate_tenant(
            db.clone(),
            org,
            set.id,
            version2.id,
            serde_json::from_value(json!({"locationId":f.venue,"at":Utc::now()})).unwrap(),
            "UTC",
            "INR",
        )
        .await
        .unwrap();
    pricing
        .publish_tenant(
            db.clone(),
            org,
            set.id,
            version2.id,
            serde_json::from_value(json!({})).unwrap(),
            f.player,
        )
        .await
        .unwrap();
    let ended = f
        .sessions
        .end_tenant(
            db.clone(),
            session.id,
            serde_json::from_value(
                json!({"reason":"voluntary","endTime":at+Duration::minutes(60)}),
            )
            .unwrap(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(ended.time_credits_consumed, Some(75));
    assert_eq!(ended.deduction_profile_snapshot, Some(snapshot));
    f.close().await;
}

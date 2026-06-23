//! Integration tests for balance raw Redis cache invalidation on recharge/deduct.
//! Run with: `cargo test --test balance_cache_invalidation -- --ignored`

use gaming_cafe_api::app::build_state;
use gaming_cafe_api::cache::keys;
use gaming_cafe_api::config::load_dotenv;
use gaming_cafe_api::models::{PlayerPlanBalance, PurchaseBalanceDto};
use std::sync::Arc;
use uuid::Uuid;

async fn setup() -> Option<Arc<gaming_cafe_api::app::AppState>> {
    load_dotenv();
    if std::env::var("DATABASE_URL").is_err() && std::env::var("DB_HOST").is_err() {
        return None;
    }
    let state = build_state().await;
    if !state.cache.is_available() {
        eprintln!("skip: Redis unavailable (set REDIS_URL or start docker-compose Redis)");
        return None;
    }
    Some(state)
}

#[derive(sqlx::FromRow)]
struct ActiveBalanceRow {
    id: Uuid,
    player_id: Uuid,
    plan_id: Uuid,
    remaining_minutes: i32,
    device_type: Option<String>,
    device_sub_type: Option<String>,
    kind: String,
}

async fn find_rechargeable_balance(db: &sqlx::PgPool) -> Option<ActiveBalanceRow> {
    sqlx::query_as::<_, ActiveBalanceRow>(
        r#"
        SELECT b.id,
               b."playerId" as player_id,
               b."sourcePlanId" as plan_id,
               b."remainingMinutes" as remaining_minutes,
               b."deviceType"::text as device_type,
               b."deviceSubType"::text as device_sub_type,
               b.kind::text as kind
        FROM player_plan_balances b
        INNER JOIN plans p ON p.id = b."sourcePlanId" AND p."deletedAt" IS NULL
        WHERE b.status = 'active'
          AND b."deletedAt" IS NULL
          AND b."remainingMinutes" > 0
          AND b."sourcePlanId" IS NOT NULL
        LIMIT 1
        "#,
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

fn balance_scope_key(row: &ActiveBalanceRow) -> String {
    keys::balance_active(
        &row.player_id,
        &format!(
            "{}:{}:{}",
            row.device_type.as_deref().unwrap_or("null"),
            row.device_sub_type.as_deref().unwrap_or("null"),
            row.kind
        ),
    )
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and REDIS_URL"]
async fn purchase_or_recharge_write_through_balance_raw_cache() {
    let Some(state) = setup().await else {
        return;
    };

    let Some(row) = find_rechargeable_balance(&state.db).await else {
        eprintln!("skip: no active balance with plan in database");
        return;
    };

    let cache_key = keys::balance_raw(&row.id);
    state
        .cache
        .delete(&[&cache_key])
        .await
        .expect("clear balance cache");

    let warmed = state.balances.get_raw(row.id).await.expect("warm cache");
    assert_eq!(warmed.remaining_minutes, row.remaining_minutes);
    assert!(
        state
            .cache
            .get_value(&cache_key)
            .await
            .expect("redis read")
            .is_some(),
        "expected balance:raw to be cached after get_raw"
    );

    let plan = state.plans.get_by_id(row.plan_id).await.expect("load plan");
    let credits = plan.time_credits;

    let updated = state
        .balances
        .purchase_or_recharge(
            PurchaseBalanceDto {
                player_id: row.player_id,
                plan_id: row.plan_id,
                transaction_id: None,
            },
            None,
        )
        .await
        .expect("recharge balance");

    assert!(
        updated.remaining_minutes >= row.remaining_minutes + credits
            || updated.remaining_minutes > row.remaining_minutes,
        "recharge should increase remaining minutes"
    );

    let cached = state
        .cache
        .get_value(&cache_key)
        .await
        .expect("redis read")
        .expect("expected balance:raw write-through after recharge");
    let cached_balance: PlayerPlanBalance =
        serde_json::from_value(cached).expect("deserialize cached balance");
    assert_eq!(
        cached_balance.remaining_minutes,
        updated.remaining_minutes,
        "write-through cache should match post-recharge balance"
    );

    let fresh = state.balances.get_raw(row.id).await.expect("reload balance");
    assert_eq!(fresh.remaining_minutes, updated.remaining_minutes);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and REDIS_URL"]
async fn purchase_or_recharge_invalidates_balance_active_scope_cache() {
    let Some(state) = setup().await else {
        return;
    };

    let Some(row) = find_rechargeable_balance(&state.db).await else {
        eprintln!("skip: no active balance with plan in database");
        return;
    };

    let scope_key = balance_scope_key(&row);
    state
        .cache
        .set_value(
            &scope_key,
            &serde_json::json!({"stale": true}),
            keys::ttl::SESSION,
        )
        .await
        .expect("seed scope cache");

    let _updated = state
        .balances
        .purchase_or_recharge(
            PurchaseBalanceDto {
                player_id: row.player_id,
                plan_id: row.plan_id,
                transaction_id: None,
            },
            None,
        )
        .await
        .expect("recharge balance");

    assert!(
        state
            .cache
            .get_value(&scope_key)
            .await
            .expect("redis read")
            .is_none(),
        "expected balance:active scope cache to be busted after recharge"
    );
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and REDIS_URL"]
async fn purchase_or_recharge_overwrites_stale_warmed_cache() {
    let Some(state) = setup().await else {
        return;
    };

    let Some(row) = find_rechargeable_balance(&state.db).await else {
        eprintln!("skip: no active balance with plan in database");
        return;
    };

    let cache_key = keys::balance_raw(&row.id);
    let stale_minutes = row.remaining_minutes.saturating_sub(999).max(0);
    let stale_balance = PlayerPlanBalance {
        remaining_minutes: stale_minutes,
        ..state.balances.get_raw(row.id).await.expect("load balance")
    };
    gaming_cafe_api::cache::set_json(
        &*state.cache,
        &cache_key,
        &stale_balance,
        keys::ttl::SESSION,
    )
    .await
    .expect("seed stale cache");

    let updated = state
        .balances
        .purchase_or_recharge(
            PurchaseBalanceDto {
                player_id: row.player_id,
                plan_id: row.plan_id,
                transaction_id: None,
            },
            None,
        )
        .await
        .expect("recharge balance");

    assert_ne!(
        updated.remaining_minutes, stale_minutes,
        "recharge should not preserve stale cached minutes"
    );

    let fresh = state.balances.get_raw(row.id).await.expect("reload balance");
    assert_eq!(
        fresh.remaining_minutes, updated.remaining_minutes,
        "get_raw should return post-recharge value, not stale cache"
    );
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and REDIS_URL"]
async fn deduct_minutes_write_through_balance_raw_cache() {
    let Some(state) = setup().await else {
        return;
    };

    let Some(row) = find_rechargeable_balance(&state.db).await else {
        eprintln!("skip: no active balance with plan in database");
        return;
    };

    if row.remaining_minutes < 2 {
        eprintln!("skip: balance has insufficient minutes for deduct test");
        return;
    }

    let cache_key = keys::balance_raw(&row.id);
    state
        .cache
        .delete(&[&cache_key])
        .await
        .expect("clear balance cache");

    let warmed = state.balances.get_raw(row.id).await.expect("warm cache");
    assert_eq!(warmed.remaining_minutes, row.remaining_minutes);
    assert!(
        state
            .cache
            .get_value(&cache_key)
            .await
            .expect("redis read")
            .is_some()
    );

    let deduct = 1;
    let updated = state
        .balances
        .deduct_minutes(row.id, deduct, None)
        .await
        .expect("deduct minutes");

    assert_eq!(updated.remaining_minutes, row.remaining_minutes - deduct);

    let cached = state
        .cache
        .get_value(&cache_key)
        .await
        .expect("redis read")
        .expect("expected balance:raw write-through after deduct_minutes");
    let cached_balance: PlayerPlanBalance =
        serde_json::from_value(cached).expect("deserialize cached balance");
    assert_eq!(
        cached_balance.remaining_minutes,
        updated.remaining_minutes,
        "write-through cache should match post-deduct balance"
    );

    let fresh = state.balances.get_raw(row.id).await.expect("reload balance");
    assert_eq!(fresh.remaining_minutes, updated.remaining_minutes);
}

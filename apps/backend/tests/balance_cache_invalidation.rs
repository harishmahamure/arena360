//! Integration tests for balance raw Redis cache invalidation on recharge/deduct.
//! Run with: `cargo test --test balance_cache_invalidation -- --ignored`

use gaming_cafe_api::app::build_state;
use gaming_cafe_api::cache::keys;
use gaming_cafe_api::config::load_dotenv;
use gaming_cafe_api::models::PurchaseBalanceDto;
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
}

async fn find_rechargeable_balance(db: &sqlx::PgPool) -> Option<ActiveBalanceRow> {
    sqlx::query_as::<_, ActiveBalanceRow>(
        r#"
        SELECT b.id,
               b."playerId" as player_id,
               b."planId" as plan_id,
               b."remainingMinutes" as remaining_minutes
        FROM player_plan_balances b
        INNER JOIN plans p ON p.id = b."planId" AND p."isActive" = true
        WHERE b.status = 'ACTIVE'
          AND b."deletedAt" IS NULL
          AND b."remainingMinutes" > 0
        LIMIT 1
        "#,
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and REDIS_URL"]
async fn purchase_or_recharge_invalidates_balance_raw_cache() {
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

    assert!(
        state
            .cache
            .get_value(&cache_key)
            .await
            .expect("redis read")
            .is_none(),
        "expected balance:raw to be busted after recharge"
    );

    let fresh = state.balances.get_raw(row.id).await.expect("reload balance");
    assert_eq!(fresh.remaining_minutes, updated.remaining_minutes);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and REDIS_URL"]
async fn deduct_minutes_invalidates_balance_raw_cache() {
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

    assert!(
        state
            .cache
            .get_value(&cache_key)
            .await
            .expect("redis read")
            .is_none(),
        "expected balance:raw to be busted after deduct_minutes"
    );

    let fresh = state.balances.get_raw(row.id).await.expect("reload balance");
    assert_eq!(fresh.remaining_minutes, updated.remaining_minutes);
}

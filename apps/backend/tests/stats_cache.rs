//! Integration tests for dashboard stats Redis caching (DRAFT-0043).
//! Run with: `cargo test --test stats_cache -- --ignored`

use gaming_cafe_api::app::build_state;
use gaming_cafe_api::cache::{self, keys};
use gaming_cafe_api::config::load_dotenv;
use std::sync::Arc;

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

#[tokio::test]
#[ignore = "requires DATABASE_URL and REDIS_URL"]
async fn dashboard_stats_populates_and_reuses_cache() {
    let Some(state) = setup().await else {
        return;
    };

    let start = "2026-06-01".to_string();
    let end = "2026-06-20".to_string();
    let cache_key = keys::stats_dashboard(&keys::filter_hash(&StatsDashboardKey {
        start: start.clone(),
        end: end.clone(),
    }));

    let first = state
        .stats
        .get_dashboard_stats(Some(start.clone()), Some(end.clone()))
        .await
        .expect("dashboard stats");

    assert!(
        state
            .cache
            .get_value(&cache_key)
            .await
            .expect("redis read")
            .is_some(),
        "expected dashboard stats cached under {cache_key}"
    );

    let second = state
        .stats
        .get_dashboard_stats(Some(start), Some(end))
        .await
        .expect("cached dashboard stats");

    assert_eq!(first.users.total_users, second.users.total_users);
    assert_eq!(
        first.revenue.current.total,
        second.revenue.current.total
    );
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and REDIS_URL"]
async fn invalidate_stats_clears_dashboard_cache() {
    let Some(state) = setup().await else {
        return;
    };

    let start = "2026-06-01".to_string();
    let end = "2026-06-20".to_string();
    let cache_key = keys::stats_dashboard(&keys::filter_hash(&StatsDashboardKey {
        start: start.clone(),
        end: end.clone(),
    }));

    state
        .stats
        .get_dashboard_stats(Some(start.clone()), Some(end.clone()))
        .await
        .expect("dashboard stats");

    assert!(
        state
            .cache
            .get_value(&cache_key)
            .await
            .expect("redis read")
            .is_some()
    );

    cache::invalidate_stats(&*state.cache)
        .await
        .expect("invalidate stats");

    assert!(
        state
            .cache
            .get_value(&cache_key)
            .await
            .expect("redis read")
            .is_none(),
        "expected stats cache busted after invalidate_stats"
    );
}

#[derive(serde::Serialize)]
struct StatsDashboardKey {
    start: String,
    end: String,
}

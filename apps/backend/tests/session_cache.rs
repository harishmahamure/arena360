//! Integration tests for session list Redis caching.
//! Run with: `cargo test --test session_cache -- --ignored`

use gaming_cafe_api::app::build_state;
use gaming_cafe_api::cache::keys;
use gaming_cafe_api::config::load_dotenv;
use gaming_cafe_api::models::SessionFilterDto;
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

fn active_list_filters() -> SessionFilterDto {
    SessionFilterDto {
        is_active: Some(1),
        ..Default::default()
    }
}

#[derive(sqlx::FromRow)]
struct OpenSessionRow {
    session_id: Uuid,
    balance_id: Uuid,
    player_id: Uuid,
}

async fn find_open_session(db: &sqlx::PgPool) -> Option<OpenSessionRow> {
    sqlx::query_as::<_, OpenSessionRow>(
        r#"
        SELECT s.id as session_id,
               s."balanceId" as balance_id,
               b."playerId" as player_id
        FROM usage_sessions s
        INNER JOIN player_plan_balances b ON b.id = s."balanceId" AND b."deletedAt" IS NULL
        WHERE s."endTime" IS NULL
          AND s."deletedAt" IS NULL
          AND b."remainingMinutes" > 1
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
async fn list_active_populates_and_reuses_cache() {
    let Some(state) = setup().await else {
        return;
    };

    let filters = active_list_filters();
    let cache_key = keys::sessions_list(&keys::filter_hash(&filters));

    let first = state
        .sessions
        .list_active()
        .await
        .expect("list active sessions");
    assert!(
        state
            .cache
            .get_value(&cache_key)
            .await
            .expect("redis read")
            .is_some(),
        "expected list result to be cached under {cache_key}"
    );

    let second = state
        .sessions
        .list_active()
        .await
        .expect("cached list active sessions");
    assert_eq!(first.total, second.total);
    assert_eq!(first.data.len(), second.data.len());
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and REDIS_URL"]
async fn session_heartbeat_invalidates_list_cache() {
    let Some(state) = setup().await else {
        return;
    };

    let Some(open) = find_open_session(&state.db).await else {
        eprintln!("skip: no open session with balance in database");
        return;
    };

    let filters = active_list_filters();
    let cache_key = keys::sessions_list(&keys::filter_hash(&filters));

    let _ = state.sessions.list_active().await.expect("warm list cache");
    assert!(
        state
            .cache
            .get_value(&cache_key)
            .await
            .expect("redis read")
            .is_some()
    );

    let session = state
        .sessions
        .get_by_id(open.session_id)
        .await
        .expect("load session");

    state
        .sessions
        .heartbeat_for_player(open.session_id, open.player_id, session.device_id)
        .await
        .expect("heartbeat session");

    assert!(
        state
            .cache
            .get_value(&cache_key)
            .await
            .expect("redis read")
            .is_none(),
        "expected sessions:list keys to be busted after heartbeat"
    );
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and REDIS_URL"]
async fn balance_mutation_invalidates_list_cache() {
    let Some(state) = setup().await else {
        return;
    };

    let Some(open) = find_open_session(&state.db).await else {
        eprintln!("skip: no open session with balance in database");
        return;
    };

    let filters = active_list_filters();
    let cache_key = keys::sessions_list(&keys::filter_hash(&filters));

    let _ = state.sessions.list_active().await.expect("warm list cache");
    assert!(
        state
            .cache
            .get_value(&cache_key)
            .await
            .expect("redis read")
            .is_some()
    );

    state
        .balances
        .deduct_minutes(open.balance_id, 1, Some(open.session_id))
        .await
        .expect("deduct minutes");

    assert!(
        state
            .cache
            .get_value(&cache_key)
            .await
            .expect("redis read")
            .is_none(),
        "expected sessions:list keys to be busted after balance mutation"
    );
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and REDIS_URL"]
async fn heartbeat_write_through_session_enriched_cache() {
    let Some(state) = setup().await else {
        return;
    };

    let Some(open) = find_open_session(&state.db).await else {
        eprintln!("skip: no open session with balance in database");
        return;
    };

    let enriched_key = keys::session_enriched(&open.session_id);
    let session = state
        .sessions
        .get_by_id(open.session_id)
        .await
        .expect("load session");

    state
        .sessions
        .heartbeat_for_player(open.session_id, open.player_id, session.device_id)
        .await
        .expect("heartbeat session");

    assert!(
        state
            .cache
            .get_value(&enriched_key)
            .await
            .expect("redis read")
            .is_some(),
        "expected session:enriched write-through after heartbeat"
    );
}

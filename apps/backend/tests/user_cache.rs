//! Integration tests for user list and auth Redis caching (DRAFT-0043 Phase 3).
//! Run with: `cargo test --test user_cache -- --ignored`

use gaming_cafe_api::app::build_state;
use gaming_cafe_api::cache::keys;
use gaming_cafe_api::config::load_dotenv;
use gaming_cafe_api::models::{UpdateUserDto, UserFilterDto};
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
async fn user_list_populates_and_reuses_cache() {
    let Some(state) = setup().await else {
        return;
    };

    let filters = UserFilterDto {
        role: Some("player".to_string()),
        page: Some(1),
        limit: Some(10),
        ..Default::default()
    };
    let cache_key = keys::users_list(&keys::filter_hash(&filters));

    let first = state.users.list(filters.clone()).await.expect("list users");
    assert!(
        state
            .cache
            .get_value(&cache_key)
            .await
            .expect("redis read")
            .is_some(),
        "expected list result to be cached under {cache_key}"
    );

    let second = state.users.list(filters).await.expect("cached list");
    assert_eq!(first.total, second.total);
    assert_eq!(first.data.len(), second.data.len());
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and REDIS_URL"]
async fn user_update_invalidates_list_cache() {
    let Some(state) = setup().await else {
        return;
    };

    let filters = UserFilterDto {
        role: Some("player".to_string()),
        page: Some(1),
        limit: Some(1),
        ..Default::default()
    };
    let cache_key = keys::users_list(&keys::filter_hash(&filters));

    let listed = state.users.list(filters.clone()).await.expect("list users");
    let Some(player) = listed.data.first() else {
        eprintln!("skip: no players in database");
        return;
    };

    let original_first_name = player.first_name.clone();

    assert!(
        state
            .cache
            .get_value(&cache_key)
            .await
            .expect("redis read")
            .is_some()
    );

    state
        .users
        .update(
            player.id,
            UpdateUserDto {
                username: None,
                phone_number: None,
                first_name: Some("CacheInvTest".to_string()),
                last_name: None,
                role: None,
                is_active: None,
            },
            None,
        )
        .await
        .expect("update player");

    assert!(
        state
            .cache
            .get_value(&cache_key)
            .await
            .expect("redis read")
            .is_none(),
        "expected users:list keys to be busted after update"
    );

    state
        .users
        .update(
            player.id,
            UpdateUserDto {
                username: None,
                phone_number: None,
                first_name: original_first_name,
                last_name: None,
                role: None,
                is_active: None,
            },
            None,
        )
        .await
        .expect("restore player first name");
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and REDIS_URL"]
async fn auth_username_lookup_populates_cache() {
    let Some(state) = setup().await else {
        return;
    };

    let row = sqlx::query_as::<_, (String,)>(
        r#"SELECT username FROM users
           WHERE role IN ('staff', 'admin') AND "isActive" = true AND "deletedAt" IS NULL
           LIMIT 1"#,
    )
    .fetch_optional(&state.db)
    .await
    .expect("query staff");

    let Some((username,)) = row else {
        eprintln!("skip: no active staff/admin in database");
        return;
    };

    let cache_key = keys::user_username(&username);

    state
        .cache
        .delete(&[&cache_key])
        .await
        .expect("clear auth cache");

    state
        .users
        .find_by_username_for_auth(&username)
        .await
        .expect("auth lookup")
        .expect("user exists");

    let cached_value = state
        .cache
        .get_value(&cache_key)
        .await
        .expect("redis read")
        .expect("expected auth profile cached under {cache_key}");
    assert!(
        cached_value.get("password_hash").and_then(|v| v.as_str()).is_some(),
        "auth cache must include password_hash for login"
    );

    let id_key = state
        .users
        .find_by_username_for_auth(&username)
        .await
        .expect("cached auth lookup")
        .expect("user exists")
        .id;
    let id_cache_key = keys::user_id(&id_key);
    assert!(
        state
            .cache
            .get_value(&id_cache_key)
            .await
            .expect("redis read")
            .is_some(),
        "expected public profile warmed at {id_cache_key}"
    );
}

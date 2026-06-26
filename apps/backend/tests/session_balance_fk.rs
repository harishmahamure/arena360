//! Integration tests for usage_sessions balanceId integrity.
//! Run with: `cargo test --test session_balance_fk -- --ignored`

use gaming_cafe_api::app::build_state;
use gaming_cafe_api::config::load_dotenv;
use std::sync::Arc;

async fn setup() -> Option<Arc<gaming_cafe_api::app::AppState>> {
    load_dotenv();
    if std::env::var("DATABASE_URL").is_err() && std::env::var("DB_HOST").is_err() {
        return None;
    }
    Some(build_state().await)
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn active_sessions_have_non_null_balance_id() {
    let Some(state) = setup().await else {
        return;
    };

    let null_count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM usage_sessions
        WHERE "deletedAt" IS NULL
          AND "endTime" IS NULL
          AND "balanceId" IS NULL
        "#,
    )
    .fetch_one(&state.db)
    .await
    .expect("count null balanceId");

    assert_eq!(
        null_count.0, 0,
        "open sessions must have balanceId set (run repair migration)"
    );
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn enriched_session_omits_balance_when_wallet_missing() {
    let Some(state) = setup().await else {
        return;
    };

    let orphan_id = uuid::Uuid::new_v4();
    let device_id: Option<(uuid::Uuid,)> = sqlx::query_as(
        r#"SELECT id FROM devices WHERE "deletedAt" IS NULL LIMIT 1"#,
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let Some((device_id,)) = device_id else {
        eprintln!("skip: no device fixture");
        return;
    };

    // Insert a row with a non-existent balanceId (simulates broken FK before repair).
    // This test is only valid when balanceId is still nullable in the test DB schema.
    let inserted = sqlx::query(
        r#"
        INSERT INTO usage_sessions (
            id, "balanceId", "deviceId", "startTime", "endTime", "createdAt", "updatedAt"
        )
        VALUES ($1, $2, $3, NOW() - INTERVAL '1 hour', NOW(), NOW(), NOW())
        "#,
    )
    .bind(orphan_id)
    .bind(uuid::Uuid::new_v4())
    .bind(device_id)
    .execute(&state.db)
    .await;

    if inserted.is_err() {
        eprintln!("skip: balanceId NOT NULL already enforced");
        return;
    }

    let enriched = state
        .sessions
        .get_by_id(orphan_id)
        .await
        .expect("load enriched session");

    assert!(
        enriched.balance.is_none(),
        "broken balanceId must not fabricate a balance summary"
    );

    let _ = sqlx::query(r#"DELETE FROM usage_sessions WHERE id = $1"#)
        .bind(orphan_id)
        .execute(&state.db)
        .await;
}

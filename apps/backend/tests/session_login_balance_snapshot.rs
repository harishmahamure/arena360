//! Integration tests for session login balance snapshot (ADR-0050).
//! Run with: `cargo test --test session_login_balance_snapshot -- --ignored`

use gaming_cafe_api::app::build_state;
use gaming_cafe_api::config::load_dotenv;
use gaming_cafe_api::models::{CreateSessionDto, PurchaseBalanceDto, UpdateDeviceStatusDto};
use std::sync::Arc;
use uuid::Uuid;

async fn setup() -> Option<Arc<gaming_cafe_api::app::AppState>> {
    load_dotenv();
    if std::env::var("DATABASE_URL").is_err() && std::env::var("DB_HOST").is_err() {
        return None;
    }
    Some(build_state().await)
}

#[derive(sqlx::FromRow)]
struct StartFixture {
    balance_id: Uuid,
    player_id: Uuid,
    plan_id: Uuid,
    device_id: Uuid,
    remaining_minutes: i32,
    plan_minutes: i32,
}

async fn find_start_fixture(db: &sqlx::PgPool) -> Option<StartFixture> {
    sqlx::query_as::<_, StartFixture>(
        r#"
        SELECT b.id as balance_id,
               b."playerId" as player_id,
               b."sourcePlanId" as plan_id,
               d.id as device_id,
               b."remainingMinutes" as remaining_minutes,
               p."timeCredits" as plan_minutes
        FROM player_plan_balances b
        INNER JOIN plans p ON p.id = b."sourcePlanId" AND p."deletedAt" IS NULL
        INNER JOIN devices d ON d."deviceType" IS NOT DISTINCT FROM b."deviceType"
                            AND d."deviceSubType" IS NOT DISTINCT FROM b."deviceSubType"
                            AND d."deletedAt" IS NULL
                            AND d.status IN ('available', 'operational')
        WHERE b.status = 'active'
          AND b."deletedAt" IS NULL
          AND b."remainingMinutes" > 0
          AND b."sourcePlanId" IS NOT NULL
          AND NOT EXISTS (
              SELECT 1 FROM usage_sessions s
              WHERE s."balanceId" = b.id
                AND s."endTime" IS NULL
                AND s."deletedAt" IS NULL
          )
          AND NOT EXISTS (
              SELECT 1 FROM usage_sessions s2
              WHERE s2."deviceId" = d.id
                AND s2."endTime" IS NULL
                AND s2."deletedAt" IS NULL
          )
        LIMIT 1
        "#,
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn session_snapshot_preserved_after_recharge() {
    let Some(state) = setup().await else {
        return;
    };

    let Some(fixture) = find_start_fixture(&state.db).await else {
        eprintln!("skip: no idle balance + device fixture");
        return;
    };

    let login_minutes = fixture.remaining_minutes;

    let session = state
        .sessions
        .start(
            CreateSessionDto {
                balance_id: fixture.balance_id,
                device_id: fixture.device_id,
                shift_id: None,
                start_time: None,
            },
            None,
        )
        .await
        .expect("start session");

    assert_eq!(
        session.wallet_minutes_at_start,
        Some(login_minutes),
        "wallet snapshot should match balance at login"
    );
    assert_eq!(
        session.source_plan_id_at_start,
        Some(fixture.plan_id),
        "plan snapshot should match source plan at login"
    );

    let updated = state
        .balances
        .purchase_or_recharge(
            PurchaseBalanceDto {
                player_id: fixture.player_id,
                plan_id: fixture.plan_id,
                transaction_id: None,
            },
            None,
        )
        .await
        .expect("recharge balance");

    assert!(
        updated.remaining_minutes >= fixture.plan_minutes,
        "live balance should reflect recharge"
    );

    let enriched = state
        .sessions
        .get_by_id(session.id)
        .await
        .expect("load enriched session");

    assert_eq!(
        enriched.wallet_minutes_at_start,
        Some(login_minutes),
        "snapshot must stay at login value after recharge"
    );
    assert_eq!(
        enriched.balance.as_ref().map(|b| b.remaining_minutes),
        Some(updated.remaining_minutes),
        "live balance on session should match wallet after recharge"
    );

    // Cleanup: end session and restore device status.
    let _ = state
        .sessions
        .end(
            session.id,
            gaming_cafe_api::models::EndSessionDto {
                reason: Some("force".to_string()),
                ..Default::default()
            },
            None,
        )
        .await;
    let _ = state
        .devices
        .update_status(
            fixture.device_id,
            UpdateDeviceStatusDto {
                status: "available".to_string(),
            },
        )
        .await;
}

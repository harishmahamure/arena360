//! Integration tests for the kiosk session endpoints (ADR-0017).
//! Run DB-backed cases with: `cargo test --test kiosk_session_flow -- --ignored`
//!
//! These mirror `player_auth_login.rs`: without `DATABASE_URL`/`JWT_SECRET` the
//! harness returns early so the suite is a no-op locally and exercised on CI/QA
//! against a seeded database.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use gaming_cafe_api::app::{build_router, build_state};
use gaming_cafe_api::config::load_dotenv;
use gaming_cafe_api::models::session::SESSION_END_REASONS;
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

async fn setup() -> Option<Arc<gaming_cafe_api::app::AppState>> {
    load_dotenv();
    if std::env::var("DATABASE_URL").is_err()
        && std::env::var("DB_HOST").is_err()
        && std::env::var("JWT_SECRET").is_err()
    {
        return None;
    }
    Some(build_state().await)
}

#[test]
fn end_reason_enum_matches_contract() {
    // The kiosk and admin send these reasons; the service validates against this set.
    assert!(SESSION_END_REASONS.contains(&"voluntary"));
    assert!(SESSION_END_REASONS.contains(&"auto"));
    assert!(SESSION_END_REASONS.contains(&"force"));
    assert!(SESSION_END_REASONS.contains(&"offline_reconcile"));
    assert!(!SESSION_END_REASONS.contains(&"bogus"));
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and JWT_SECRET"]
async fn start_session_requires_player_auth() {
    let Some(state) = setup().await else {
        return;
    };
    let app = build_router(state);

    // No device bearer + no X-Player-Token => unauthorized.
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/kiosk/sessions")
                .header("content-type", "application/json")
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and a seeded player with two registered devices"]
async fn second_device_start_returns_409() {
    let Some(state) = setup().await else {
        return;
    };

    // Find an active player and a registered device.
    let player = sqlx::query_as::<_, (Uuid, String)>(
        r#"SELECT id, username FROM users WHERE role = 'player' AND "isActive" = true AND "deletedAt" IS NULL LIMIT 1"#,
    )
    .fetch_optional(&state.db)
    .await
    .expect("query player");
    let Some((player_id, _username)) = player else {
        eprintln!("skip: no active player");
        return;
    };

    let devices =
        sqlx::query_as::<_, (Uuid,)>(r#"SELECT id FROM devices WHERE "deletedAt" IS NULL LIMIT 2"#)
            .fetch_all(&state.db)
            .await
            .expect("query devices");
    if devices.len() < 2 {
        eprintln!("skip: need two devices");
        return;
    }
    let (device_a,) = devices[0];
    let (device_b,) = devices[1];

    for (id,) in &devices {
        sqlx::query(
            r#"UPDATE devices SET "registrationStatus" = 'registered'::devices_registrationstatus_enum WHERE id = $1"#,
        )
        .bind(id)
        .execute(&state.db)
        .await
        .expect("register device");
    }

    let player_row = state.users.get_by_id(player_id).await.expect("load player");
    let token_a = state
        .auth
        .generate_player_token(&player_row, device_a)
        .expect("player token a");
    let token_b = state
        .auth
        .generate_player_token(&player_row, device_b)
        .expect("player token b");
    let bearer_a = state
        .auth
        .generate_device_token(device_a)
        .expect("device a");
    let bearer_b = state
        .auth
        .generate_device_token(device_b)
        .expect("device b");

    let app = build_router(state.clone());

    // Start on device A.
    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/kiosk/sessions")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {bearer_a}"))
                .header("x-player-token", token_a)
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        first.status() == StatusCode::CREATED || first.status() == StatusCode::FORBIDDEN,
        "expected start or balance-gate, got {}",
        first.status()
    );
    if first.status() != StatusCode::CREATED {
        eprintln!("skip: player has no usable balance to start");
        return;
    }

    // Start on device B => single-session conflict.
    let second = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/kiosk/sessions")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {bearer_b}"))
                .header("x-player-token", token_b)
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CONFLICT);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(second.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["message"], "PLAYER_ALREADY_IN_SESSION");
}

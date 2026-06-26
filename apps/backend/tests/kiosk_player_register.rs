//! Integration tests for POST /auth/register/player (DRAFT-0049).
//! Run with: `cargo test --test kiosk_player_register -- --ignored`

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use gaming_cafe_api::app::{build_router, build_state};
use gaming_cafe_api::config::load_dotenv;
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

#[tokio::test]
#[ignore = "requires DATABASE_URL and seeded device rows"]
async fn register_player_returns_401_without_device_token() {
    let Some(state) = setup().await else {
        return;
    };
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register/player")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "newplayer",
                        "password": "password123",
                        "phoneNumber": "9876543210"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and seeded device rows"]
async fn register_player_validation_and_conflict_paths() {
    let Some(state) = setup().await else {
        return;
    };

    let device_row =
        sqlx::query_as::<_, (Uuid,)>(r#"SELECT id FROM devices WHERE "deletedAt" IS NULL LIMIT 1"#)
            .fetch_optional(&state.db)
            .await
            .expect("query device");

    let Some((device_id,)) = device_row else {
        eprintln!("skip: no device in database");
        return;
    };

    let existing_player = sqlx::query_as::<_, (String,)>(
        r#"SELECT username FROM users WHERE role = 'player' AND "deletedAt" IS NULL LIMIT 1"#,
    )
    .fetch_optional(&state.db)
    .await
    .expect("query player");

    sqlx::query(
        r#"UPDATE devices SET "registrationStatus" = 'registered'::devices_registrationstatus_enum, status = 'operational'::devices_status_enum WHERE id = $1"#,
    )
    .bind(device_id)
    .execute(&state.db)
    .await
    .expect("mark device registered");

    let device_token = state
        .auth
        .generate_device_token(device_id)
        .expect("device token");

    let app = build_router(state.clone());

    // 400 weak password
    let weak_pw = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register/player")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {device_token}"))
                .body(Body::from(
                    json!({
                        "username": "brandnewplayer",
                        "password": "short",
                        "phoneNumber": "9876543210"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(weak_pw.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(weak_pw.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["message"], "AUTH_WEAK_PASSWORD");
    assert_eq!(body["details"]["field"], "password");

    if let Some((existing_username,)) = existing_player {
        let conflict = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/register/player")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {device_token}"))
                    .body(Body::from(
                        json!({
                            "username": existing_username,
                            "password": "password123",
                            "phoneNumber": "9876543210"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(conflict.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["message"], "AUTH_USERNAME_ALREADY_EXISTS");
        assert_eq!(body["details"]["field"], "username");
    }

    // 403 unregistered device
    sqlx::query(
        r#"UPDATE devices SET "registrationStatus" = 'unregistered'::devices_registrationstatus_enum WHERE id = $1"#,
    )
    .bind(device_id)
    .execute(&state.db)
    .await
    .expect("mark unregistered");

    let unreg = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register/player")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {device_token}"))
                .body(Body::from(
                    json!({
                        "username": "anothernewplayer",
                        "password": "password123",
                        "phoneNumber": "9876543210"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unreg.status(), StatusCode::FORBIDDEN);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(unreg.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["message"], "DEVICE_NOT_REGISTERED");
}

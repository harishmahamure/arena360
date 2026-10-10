//! Kiosk lifecycle through the HTTP router, with isolated tenant wallets and devices.
mod support;
use axum::http::StatusCode;
use gaming_cafe_api::{
    app::build_router, models::session::SESSION_END_REASONS, repositories::TenantUserRepository,
};
use serde_json::json;
use support::{request, SessionFixture};
#[test]
fn end_reason_enum_matches_contract() {
    for reason in ["voluntary", "auto", "force", "offline_reconcile"] {
        assert!(SESSION_END_REASONS.contains(&reason));
    }
    assert!(!SESSION_END_REASONS.contains(&"bogus"));
}
#[tokio::test]
async fn start_session_requires_player_auth() {
    let f = SessionFixture::new().await;
    assert_eq!(
        request(
            build_router(f.app().await),
            "POST",
            "/kiosk/sessions",
            None,
            None,
            json!({})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    f.close().await;
}
#[tokio::test]
async fn second_device_start_returns_409_and_current_session_can_end() {
    let f = SessionFixture::new().await;
    let state = f.app().await;
    let second = f.second_device().await;
    let org = f.tenant.db.tenant_id();
    let player = TenantUserRepository::new(f.tenant.db.clone())
        .find_by_id(f.player)
        .await
        .unwrap()
        .unwrap();
    let bearer = state
        .auth
        .generate_device_token_for(f.device, org, f.venue)
        .unwrap();
    let bearer2 = state
        .auth
        .generate_device_token_for(second, org, f.venue)
        .unwrap();
    let token = state
        .auth
        .generate_player_token_for(&player, f.device, org, f.venue)
        .unwrap();
    let token2 = state
        .auth
        .generate_player_token_for(&player, second, org, f.venue)
        .unwrap();
    let app = build_router(state);
    let (status, body) = request(
        app.clone(),
        "POST",
        "/kiosk/sessions",
        Some(&bearer),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let id = body["data"]["sessionId"].as_str().unwrap().to_owned();
    let (status, body) = request(
        app.clone(),
        "POST",
        "/kiosk/sessions",
        Some(&bearer2),
        Some(&token2),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["message"], "PLAYER_ALREADY_IN_SESSION");
    let (status, body) = request(
        app.clone(),
        "GET",
        "/kiosk/sessions/current",
        Some(&bearer),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["sessionId"], id);
    let (status, body) = request(
        app.clone(),
        "PATCH",
        &format!("/kiosk/sessions/{id}/heartbeat"),
        Some(&bearer),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = request(
        app.clone(),
        "PATCH",
        &format!("/kiosk/sessions/{id}/end"),
        Some(&bearer),
        Some(&token),
        json!({"reason":"voluntary"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["data"]["endTime"].is_string());
    let (status, body) = request(
        app,
        "GET",
        "/kiosk/sessions/current",
        Some(&bearer),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["data"].is_null());
    f.close().await;
}

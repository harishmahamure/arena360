//! Player login through the real HTTP router with tenant SQLite and known credentials.
mod support;
use axum::http::StatusCode;
use gaming_cafe_api::app::build_router;
use serde_json::json;
use support::{request, SessionFixture};
#[tokio::test]
async fn login_player_returns_401_without_device_token() {
    let f = SessionFixture::new().await;
    let app = build_router(f.app().await);
    assert_eq!(
        request(
            app,
            "POST",
            "/auth/login/player",
            None,
            None,
            json!({"username":"session-player","password":"initial-password"})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    f.close().await;
}
#[tokio::test]
async fn login_player_happy_and_error_paths() {
    let f = SessionFixture::new().await;
    let state = f.app().await;
    let token = state
        .auth
        .generate_device_token_for(f.device, f.tenant.db.tenant_id(), f.venue)
        .unwrap();
    let app = build_router(state);
    let (status, body) = request(
        app.clone(),
        "POST",
        "/auth/login/player",
        Some(&token),
        None,
        json!({"username":"session-player","password":"wrong-password"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["message"], "AUTH_INVALID_CREDENTIALS");
    f.registration("unregistered").await;
    let (status, body) = request(
        app.clone(),
        "POST",
        "/auth/login/player",
        Some(&token),
        None,
        json!({"username":"session-player","password":"initial-password"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["message"], "DEVICE_NOT_REGISTERED");
    f.registration("registered").await;
    let (status, body) = request(
        app,
        "POST",
        "/auth/login/player",
        Some(&token),
        None,
        json!({"username":"session-player","password":"initial-password"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["data"]["accessToken"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    f.close().await;
}

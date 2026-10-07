//! Kiosk registration has deterministic SQLite fixtures and exercises successful creation.
mod support;
use axum::http::StatusCode;
use gaming_cafe_api::app::build_router;
use serde_json::json;
use support::{request, SessionFixture};
#[tokio::test]
async fn register_player_returns_401_without_device_token() {
    let f = SessionFixture::new().await;
    assert_eq!(
        request(
            build_router(f.app().await),
            "POST",
            "/auth/register/player",
            None,
            None,
            json!({"username":"newplayer","password":"password123","phoneNumber":"9876543210"})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    f.close().await;
}
#[tokio::test]
async fn register_player_validation_conflict_and_success_paths() {
    let f = SessionFixture::new().await;
    let state = f.app().await;
    let token = state
        .auth
        .generate_device_token_for(f.device, f.tenant.db.tenant_id(), f.venue)
        .unwrap();
    let app = build_router(state);
    for (username, password, expected, message) in [
        (
            "brandnewplayer",
            "short",
            StatusCode::BAD_REQUEST,
            "AUTH_WEAK_PASSWORD",
        ),
        (
            "session-player",
            "password123",
            StatusCode::CONFLICT,
            "AUTH_USERNAME_ALREADY_EXISTS",
        ),
    ] {
        let (status, body) = request(
            app.clone(),
            "POST",
            "/auth/register/player",
            Some(&token),
            None,
            json!({"username":username,"password":password,"phoneNumber":"9876543210"}),
        )
        .await;
        assert_eq!(status, expected, "{body}");
        assert_eq!(body["message"], message);
    }
    f.registration("unregistered").await;
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/auth/register/player",
            Some(&token),
            None,
            json!({"username":"anotherplayer","password":"password123","phoneNumber":"9876543210"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    f.registration("registered").await;
    let (status, body) = request(
        app,
        "POST",
        "/auth/register/player",
        Some(&token),
        None,
        json!({"username":"newplayer","password":"password123","phoneNumber":"9876543210"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(
        gaming_cafe_api::repositories::TenantUserRepository::new(f.tenant.db.clone())
            .find_player_for_auth("newplayer")
            .await
            .unwrap()
            .is_some()
    );
    f.close().await;
}

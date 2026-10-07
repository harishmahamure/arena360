//! Kiosk and staff order endpoints require authenticated identities.
mod support;
use axum::http::StatusCode;
use gaming_cafe_api::app::build_router;
use serde_json::json;
#[tokio::test]
async fn order_routes_require_auth() {
    let f = support::SessionFixture::new().await;
    let app = build_router(f.app().await);
    for (method, path) in [
        ("GET", "/kiosk/products"),
        ("POST", "/kiosk/orders"),
        ("GET", "/kiosk-orders"),
    ] {
        assert_eq!(
            support::request(app.clone(), method, path, None, None, json!({}))
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    f.close().await;
}

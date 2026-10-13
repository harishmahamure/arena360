mod support;

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware,
    routing::get,
    Router,
};
use gaming_cafe_api::{app::build_state_with_settings, middleware::global_rate_limit};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tower::ServiceExt;

#[tokio::test]
async fn throttles_http_and_grpc_without_external_services_and_exempts_health() {
    let state = build_state_with_settings(Arc::new(support::settings())).await;
    // Fill the first IP bucket without introducing wall-clock refill into the test.
    for _ in 0..2_000 {
        assert!(
            state
                .rate_limits
                .record_attempt("rate_limit:ip:192.0.2.1", 2_000, Duration::from_secs(60))
                .allowed
        );
    }
    let app = Router::new()
        .route("/limited", get(|| async { "ok" }))
        .route("/health", get(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(state, global_rate_limit));

    for (path, ip, content_type, expected) in [
        (
            "/limited",
            "192.0.2.1:12345",
            "application/json",
            StatusCode::TOO_MANY_REQUESTS,
        ),
        (
            "/limited",
            "192.0.2.1:12345",
            "application/grpc-web+proto",
            StatusCode::OK,
        ),
        (
            "/health",
            "192.0.2.1:12345",
            "application/json",
            StatusCode::OK,
        ),
        (
            "/limited",
            "192.0.2.2:12345",
            "application/json",
            StatusCode::OK,
        ),
    ] {
        let mut request = Request::builder()
            .uri(path)
            .header("content-type", content_type)
            .body(Body::empty())
            .unwrap();
        request
            .extensions_mut()
            .insert(ConnectInfo(ip.parse::<SocketAddr>().unwrap()));
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), expected);
        if path == "/limited" && ip.starts_with("192.0.2.1:") {
            assert!(response.headers().contains_key("retry-after"));
            assert_eq!(response.headers()["x-ratelimit-remaining"], "0");
            if content_type.starts_with("application/grpc") {
                assert_eq!(response.headers()["grpc-status"], "8");
            }
        }
    }
}

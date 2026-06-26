//! Integration tests for kiosk player ordering (DRAFT-0051).
//! Run with: `cargo test --test kiosk_orders -- --ignored`

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use gaming_cafe_api::app::{build_router, build_state};
use gaming_cafe_api::config::load_dotenv;
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

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
#[ignore = "requires DATABASE_URL and seeded data"]
async fn kiosk_products_requires_auth() {
    let Some(state) = setup().await else {
        return;
    };
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/kiosk/products")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and seeded data"]
async fn place_order_requires_auth() {
    let Some(state) = setup().await else {
        return;
    };
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/kiosk/orders")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "lineItems": [{ "productId": "00000000-0000-0000-0000-000000000001", "quantity": 1 }]
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
#[ignore = "requires DATABASE_URL and seeded data"]
async fn staff_kiosk_orders_list_requires_auth() {
    let Some(state) = setup().await else {
        return;
    };
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/kiosk-orders")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

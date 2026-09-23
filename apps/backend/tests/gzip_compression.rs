use std::io::Read;

use axum::{
    body::{to_bytes, Body},
    http::{header, Request},
    routing::get,
    Router,
};
use flate2::read::GzDecoder;
use tower::ServiceExt;
use tower_http::compression::CompressionLayer;

#[tokio::test]
async fn large_http_response_is_gzip_compressed_and_round_trips() {
    let payload = "arena360 protobuf compression ".repeat(1_000);
    let expected = payload.clone();
    let app = Router::new()
        .route("/large", get(move || async move { payload }))
        .layer(CompressionLayer::new());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/large")
                .header(header::ACCEPT_ENCODING, "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.headers().get(header::CONTENT_ENCODING).unwrap(),
        "gzip"
    );

    let compressed = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(compressed.len() < expected.len());
    let mut decoder = GzDecoder::new(compressed.as_ref());
    let mut decoded = String::new();
    decoder.read_to_string(&mut decoded).unwrap();
    assert_eq!(decoded, expected);
}

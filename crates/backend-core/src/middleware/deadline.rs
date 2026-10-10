use std::time::Duration;

use axum::{
    body::Body,
    http::{header, Request, Response, StatusCode},
    middleware::Next,
};

pub async fn request_deadline(request: Request<Body>, next: Next) -> Response<Body> {
    let path = request.uri().path();
    let timeout =
        if path.starts_with("/stats") || path.starts_with("/arena360.v1.StatisticsService/") {
            Duration::from_secs(30)
        } else {
            Duration::from_secs(15)
        };
    let is_grpc = path.starts_with("/arena360.v1.");

    match tokio::time::timeout(timeout, next.run(request)).await {
        Ok(response) => response,
        Err(_) if is_grpc => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/grpc-web+proto")
            .header("grpc-status", "4")
            .header("grpc-message", "deadline exceeded")
            .body(Body::empty())
            .expect("static gRPC deadline response"),
        Err(_) => Response::builder()
            .status(StatusCode::REQUEST_TIMEOUT)
            .body(Body::from("request deadline exceeded"))
            .expect("static HTTP deadline response"),
    }
}

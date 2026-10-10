use axum::{
    body::Body,
    http::{HeaderValue, Request, Response},
    middleware::Next,
};
use tracing::Instrument;

const REQUEST_ID_HEADER: &str = "x-request-id";

pub async fn request_context(mut request: Request<Body>, next: Next) -> Response<Body> {
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty() && value.len() <= 128 && value.is_ascii())
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let header = HeaderValue::from_str(&request_id).expect("validated request id");
    request
        .headers_mut()
        .insert(REQUEST_ID_HEADER, header.clone());

    let span = tracing::info_span!("request", request_id = %request_id);
    let mut response = next.run(request).instrument(span).await;
    response.headers_mut().insert(REQUEST_ID_HEADER, header);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{middleware, routing::get, Router};
    use tower::ServiceExt;

    #[tokio::test]
    async fn creates_and_returns_request_id() {
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(middleware::from_fn(request_context));
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let request_id = response.headers().get(REQUEST_ID_HEADER).unwrap();
        assert!(uuid::Uuid::parse_str(request_id.to_str().unwrap()).is_ok());
    }

    #[tokio::test]
    async fn preserves_valid_request_id() {
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(middleware::from_fn(request_context));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(REQUEST_ID_HEADER, "trace-123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.headers()[REQUEST_ID_HEADER], "trace-123");
    }
}

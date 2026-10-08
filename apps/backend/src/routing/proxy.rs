use std::sync::Arc;

use axum::body::Body;
use axum::extract::ws::{CloseFrame, Message as AxumMessage, WebSocket, WebSocketUpgrade};
use axum::extract::FromRequestParts;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderName, Request, Response};
use axum::middleware::Next;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::{
    frame::coding::CloseCode, CloseFrame as TungsteniteCloseFrame, Message as TungsteniteMessage,
};
use uuid::Uuid;

use super::RoutingCache;
use crate::app::AppState;
use crate::dto::JwtUserClaims;
use crate::error::AppError;

pub const ROUTED_HEADER: &str = "x-arena-routed";

#[derive(Clone)]
pub struct TenantRouter {
    cache: Arc<RoutingCache>,
    local_cell: Option<Uuid>,
    http: reqwest::Client,
    cold: Option<crate::cold::Coordinator>,
}

impl TenantRouter {
    pub fn new(cache: Arc<RoutingCache>, local_cell: Option<Uuid>) -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| AppError::Internal(format!("router client: {error}")))?;
        Ok(Self {
            cache,
            local_cell,
            http,
            cold: None,
        })
    }

    pub fn with_cold(mut self, coordinator: crate::cold::Coordinator) -> Self {
        self.cold = Some(coordinator);
        self
    }
    pub async fn remote_address(&self, tenant_id: Uuid) -> Result<Option<String>, AppError> {
        let mut target = self.cache.resolve(tenant_id).await?;
        if target.is_none() {
            if let Some(cold) = &self.cold {
                cold.wake(tenant_id).await?;
                target = self.cache.refresh_tenant(tenant_id).await?;
            }
        }
        let target = target.ok_or_else(|| AppError::Forbidden("Tenant has no active storage-cell owner".into()))?;
        Ok((Some(target.owner_cell) != self.local_cell).then_some(target.address))
    }
    /// Finish authentication using the selected tenant's current grants on its owner.
    pub async fn finish_auth(
        &self,
        tenant: Uuid,
        token: &str,
        staff_shift: bool,
    ) -> Result<Option<crate::dto::AuthResponseDto>, AppError> {
        let Some(address) = self.remote_address(tenant).await? else {
            return Ok(None);
        };
        let path = if staff_shift {
            "/auth/staff-shift"
        } else {
            "/auth/refresh"
        };
        self.finish_auth_path(&address, token, path).await.map(Some)
    }
    pub async fn finish_admin_login(
        &self,
        tenant: Uuid,
        token: &str,
    ) -> Result<Option<crate::dto::AuthResponseDto>, AppError> {
        let Some(address) = self.remote_address(tenant).await? else {
            return Ok(None);
        };
        self.finish_auth_path(&address, token, "/auth/admin-shift-close")
            .await
            .map(Some)
    }
    async fn finish_auth_path(
        &self,
        address: &str,
        token: &str,
        path: &str,
    ) -> Result<crate::dto::AuthResponseDto, AppError> {
        let response = self
            .http
            .post(format!("{}{}", address.trim_end_matches('/'), path))
            .bearer_auth(token)
            .header(ROUTED_HEADER, "1")
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|error| AppError::Internal(format!("owner authentication failed: {error}")))?;
        let status = response.status();
        let body: serde_json::Value = response.json().await.map_err(|e| {
            AppError::Internal(format!("invalid owner authentication response: {e}"))
        })?;
        if !status.is_success() {
            return Err(AppError::Api {
                code: body
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("OWNER_AUTH_FAILED")
                    .into(),
                status,
                details: body.get("details").cloned(),
            });
        }
        let auth = serde_json::from_value(body.get("data").cloned().ok_or_else(|| {
            AppError::Internal("Owner authentication response missing data".into())
        })?)
        .map_err(|e| AppError::Internal(format!("invalid owner authentication payload: {e}")))?;
        Ok(auth)
    }
}

pub async fn route_tenant_request(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response<Body>, AppError> {
    let Some(router) = &state.routing else {
        return Ok(next.run(request).await);
    };
    if is_control_auth_path(request.uri().path()) {
        return Ok(next.run(request).await);
    }
    let Some(claims) = request.extensions().get::<JwtUserClaims>() else {
        return Ok(next.run(request).await);
    };
    let tenant_id = Uuid::parse_str(&claims.tenantId)
        .map_err(|_| AppError::Forbidden("Select a tenant".into()))?;
    let Some(address) = router.remote_address(tenant_id).await? else {
        return Ok(next.run(request).await);
    };
    if request.headers().contains_key(ROUTED_HEADER) {
        return Err(routing_loop_error());
    }

    if is_websocket_upgrade(request.headers()) {
        proxy_websocket(request, &address).await
    } else {
        proxy_http(&router.http, request, &address).await
    }
}

async fn proxy_http(
    client: &reqwest::Client,
    request: Request<Body>,
    address: &str,
) -> Result<Response<Body>, AppError> {
    let (parts, body) = request.into_parts();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let url = format!("{}{}", address.trim_end_matches('/'), path_and_query);
    let mut upstream = client.request(parts.method, url);
    for (name, value) in &parts.headers {
        if !is_hop_by_hop(name) && name != header::HOST {
            upstream = upstream.header(name, value);
        }
    }
    upstream = upstream.header(ROUTED_HEADER, "1");
    let response = upstream
        .body(reqwest::Body::wrap_stream(body.into_data_stream()))
        .send()
        .await
        .map_err(|error| AppError::Internal(format!("owner cell proxy failed: {error}")))?;
    let status = response.status();
    let headers = response.headers().clone();
    let mut downstream = Response::builder().status(status);
    for (name, value) in &headers {
        if !is_hop_by_hop(name) {
            downstream = downstream.header(name, value);
        }
    }
    downstream
        .body(Body::from_stream(response.bytes_stream()))
        .map_err(|error| AppError::Internal(format!("owner cell response failed: {error}")))
}

async fn proxy_websocket(
    request: Request<Body>,
    address: &str,
) -> Result<Response<Body>, AppError> {
    let (mut parts, _) = request.into_parts();
    let original_headers = parts.headers.clone();
    let upgrade = WebSocketUpgrade::from_request_parts(&mut parts, &())
        .await
        .map_err(|_| AppError::BadRequest("Invalid WebSocket upgrade".into()))?;
    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/realtime");
    proxy_websocket_upgrade(upgrade, original_headers, address, path_and_query).await
}

pub async fn proxy_websocket_upgrade(
    upgrade: WebSocketUpgrade,
    original_headers: HeaderMap,
    address: &str,
    path_and_query: &str,
) -> Result<Response<Body>, AppError> {
    let ws_base = if let Some(rest) = address.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = address.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        address.to_string()
    };
    let url = format!("{}{}", ws_base.trim_end_matches('/'), path_and_query);
    let mut upstream_request = url
        .into_client_request()
        .map_err(|error| AppError::Internal(format!("WebSocket proxy URL: {error}")))?;
    copy_header(
        &original_headers,
        upstream_request.headers_mut(),
        header::AUTHORIZATION,
    );
    copy_header(
        &original_headers,
        upstream_request.headers_mut(),
        header::SEC_WEBSOCKET_PROTOCOL,
    );
    upstream_request
        .headers_mut()
        .insert(ROUTED_HEADER, axum::http::HeaderValue::from_static("1"));

    Ok(upgrade
        .protocols(["arena360.protobuf.v1"])
        .on_upgrade(move |downstream| async move {
            match tokio_tungstenite::connect_async(upstream_request).await {
                Ok((upstream, _)) => bridge_websockets(downstream, upstream).await,
                Err(error) => tracing::warn!(%error, "Owner-cell WebSocket connection failed"),
            }
        })
        .into_response())
}

async fn bridge_websockets(
    downstream: WebSocket,
    upstream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) {
    let (mut downstream_sink, mut downstream_stream) = downstream.split();
    let (mut upstream_sink, mut upstream_stream) = upstream.split();
    loop {
        tokio::select! {
            message = downstream_stream.next() => {
                let Some(Ok(message)) = message else { break };
                if let Some(message) = to_upstream(message) {
                    if upstream_sink.send(message).await.is_err() {
                        break;
                    }
                }
            }
            message = upstream_stream.next() => {
                let Some(Ok(message)) = message else { break };
                if let Some(message) = to_downstream(message) {
                    if downstream_sink.send(message).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
    let _ = upstream_sink.close().await;
    let _ = downstream_sink.close().await;
}

fn to_upstream(message: AxumMessage) -> Option<TungsteniteMessage> {
    match message {
        AxumMessage::Text(value) => Some(TungsteniteMessage::Text(value.to_string().into())),
        AxumMessage::Binary(value) => Some(TungsteniteMessage::Binary(value.to_vec().into())),
        AxumMessage::Ping(value) => Some(TungsteniteMessage::Ping(value.to_vec().into())),
        AxumMessage::Pong(value) => Some(TungsteniteMessage::Pong(value.to_vec().into())),
        AxumMessage::Close(frame) => Some(TungsteniteMessage::Close(frame.map(|frame| {
            TungsteniteCloseFrame {
                code: CloseCode::from(frame.code),
                reason: frame.reason.to_string().into(),
            }
        }))),
    }
}

fn to_downstream(message: TungsteniteMessage) -> Option<AxumMessage> {
    match message {
        TungsteniteMessage::Text(value) => Some(AxumMessage::Text(value.to_string().into())),
        TungsteniteMessage::Binary(value) => Some(AxumMessage::Binary(value.to_vec().into())),
        TungsteniteMessage::Ping(value) => Some(AxumMessage::Ping(value.to_vec().into())),
        TungsteniteMessage::Pong(value) => Some(AxumMessage::Pong(value.to_vec().into())),
        TungsteniteMessage::Close(frame) => {
            Some(AxumMessage::Close(frame.map(|frame| CloseFrame {
                code: frame.code.into(),
                reason: frame.reason.to_string().into(),
            })))
        }
        TungsteniteMessage::Frame(_) => None,
    }
}

fn is_websocket_upgrade(headers: &HeaderMap) -> bool {
    headers
        .get(header::UPGRADE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
}

fn is_control_auth_path(path: &str) -> bool {
    matches!(
        path,
        "/auth/login/admin" | "/auth/login/staff" | "/auth/login/panel" | "/auth/login/panel/mfa"
    )
}

fn is_hop_by_hop(name: &HeaderName) -> bool {
    matches!(
        name.as_str().to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

fn copy_header(
    source: &HeaderMap,
    target: &mut tokio_tungstenite::tungstenite::http::HeaderMap,
    name: HeaderName,
) {
    if let Some(value) = source.get(&name) {
        target.insert(name, value.clone());
    }
}

pub fn routing_loop_error() -> AppError {
    AppError::Api {
        code: "ROUTING_STALE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: None,
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use axum::body::{to_bytes, Bytes};
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::routing::get;
    use axum::Router;

    use super::*;

    #[tokio::test]
    async fn http_proxy_preserves_streaming_response_headers_and_body() {
        let app = Router::new().route(
            "/events",
            get(|| async {
                let chunks = futures::stream::iter([
                    Ok::<_, Infallible>(Bytes::from_static(b"data: one\n\n")),
                    Ok::<_, Infallible>(Bytes::from_static(b"data: two\n\n")),
                ]);
                Response::builder()
                    .header(header::CONTENT_TYPE, "text/event-stream")
                    .body(Body::from_stream(chunks))
                    .unwrap()
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let request = Request::builder()
            .uri("/events")
            .body(Body::empty())
            .unwrap();
        let response = proxy_http(&reqwest::Client::new(), request, &address)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/event-stream"
        );
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(&body[..], b"data: one\n\ndata: two\n\n");
        server.abort();
    }

    #[test]
    fn websocket_frames_are_translated_in_both_directions() {
        let upstream = to_upstream(AxumMessage::Binary(vec![1, 2, 3].into())).unwrap();
        assert_eq!(upstream, TungsteniteMessage::Binary(vec![1, 2, 3].into()));

        let downstream = to_downstream(TungsteniteMessage::Text("arena".into())).unwrap();
        assert_eq!(downstream, AxumMessage::Text("arena".into()));
    }

    #[tokio::test]
    async fn websocket_proxy_bridges_binary_frames() {
        async fn echo(ws: WebSocketUpgrade) -> Response<Body> {
            ws.protocols(["arena360.protobuf.v1"])
                .on_upgrade(|socket| async move {
                    let (mut sink, mut stream) = socket.split();
                    while let Some(Ok(message)) = stream.next().await {
                        if sink.send(message).await.is_err() {
                            break;
                        }
                    }
                })
                .into_response()
        }

        let upstream_app = Router::new().route("/realtime", get(echo));
        let upstream_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_address = format!("http://{}", upstream_listener.local_addr().unwrap());
        let upstream_server = tokio::spawn(async move {
            axum::serve(upstream_listener, upstream_app).await.unwrap();
        });

        async fn route_proxy(
            State(address): State<String>,
            headers: HeaderMap,
            ws: WebSocketUpgrade,
        ) -> Result<Response<Body>, AppError> {
            proxy_websocket_upgrade(ws, headers, &address, "/realtime").await
        }
        let proxy_app = Router::new()
            .route("/realtime", get(route_proxy))
            .with_state(upstream_address);
        let proxy_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_address = proxy_listener.local_addr().unwrap();
        let proxy_server = tokio::spawn(async move {
            axum::serve(proxy_listener, proxy_app).await.unwrap();
        });

        let mut request = format!("ws://{proxy_address}/realtime")
            .into_client_request()
            .unwrap();
        request.headers_mut().insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            axum::http::HeaderValue::from_static("arena360.protobuf.v1"),
        );
        let (mut client, response) = tokio_tungstenite::connect_async(request).await.unwrap();
        assert_eq!(
            response
                .headers()
                .get(header::SEC_WEBSOCKET_PROTOCOL)
                .unwrap(),
            "arena360.protobuf.v1"
        );
        client
            .send(TungsteniteMessage::Binary(vec![7, 8, 9].into()))
            .await
            .unwrap();
        assert_eq!(
            client.next().await.unwrap().unwrap(),
            TungsteniteMessage::Binary(vec![7, 8, 9].into())
        );

        proxy_server.abort();
        upstream_server.abort();
    }
}

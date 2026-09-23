use crate::app::AppState;
use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{header, HeaderMap, HeaderValue, Method, Request, Response, StatusCode},
    middleware::Next,
};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};

const CAPACITY: u32 = 2_000;
const WINDOW: Duration = Duration::from_secs(60);

pub async fn global_rate_limit(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let path = request.uri().path();
    if request.method() == Method::OPTIONS
        || matches!(
            path,
            "/health" | "/health/live" | "/health/ready" | "/metrics" | "/realtime"
        )
    {
        return next.run(request).await;
    }
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip());
    let Some(ip) = peer.map(|peer| {
        canonical_client_ip(peer, request.headers(), &state.settings.trusted_proxy_cidrs)
    }) else {
        tracing::error!("missing socket address; global rate limiter failing open");
        state.metrics.rate_fail_open();
        return next.run(request).await;
    };
    let key = format!("rate_limit:ip:{ip}");
    match state.cache.consume_ip_token(&key, CAPACITY, WINDOW).await {
        Ok(Some(decision)) if !decision.allowed => {
            state.metrics.rate_rejected();
            rate_limited_response(
                request.headers(),
                request.uri().path(),
                decision.retry_after_ms,
                decision.remaining,
            )
        }
        Ok(Some(_)) => {
            state.metrics.rate_allowed();
            next.run(request).await
        }
        Ok(None) => {
            tracing::error!("Redis rate limiting unavailable; enforcement is fail-open");
            state.metrics.rate_fail_open();
            next.run(request).await
        }
        Err(error) => {
            tracing::error!(%error, "Redis rate limiting failed; enforcement is fail-open");
            state.metrics.rate_fail_open();
            next.run(request).await
        }
    }
}

fn rate_limited_response(
    headers: &HeaderMap,
    path: &str,
    retry_ms: u64,
    remaining: u32,
) -> Response<Body> {
    let retry_seconds = retry_ms.div_ceil(1_000).max(1);
    let is_grpc = path.starts_with("/arena360.v1.")
        || headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("application/grpc"));
    let mut response = if is_grpc {
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/grpc-web+proto")
            .header("grpc-status", "8")
            .header("grpc-message", "rate limit exceeded")
            .body(Body::empty())
            .expect("static gRPC rate-limit response")
    } else {
        Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"statusCode":429,"message":"RATE_LIMITED","error":"Too Many Requests"}"#,
            ))
            .expect("static HTTP rate-limit response")
    };
    let headers = response.headers_mut();
    headers.insert(
        header::RETRY_AFTER,
        HeaderValue::from_str(&retry_seconds.to_string()).unwrap(),
    );
    headers.insert("x-ratelimit-limit", HeaderValue::from_static("2000"));
    headers.insert(
        "x-ratelimit-remaining",
        HeaderValue::from_str(&remaining.to_string()).unwrap(),
    );
    response
}

pub fn canonical_client_ip(peer: IpAddr, headers: &HeaderMap, trusted: &[ipnet::IpNet]) -> IpAddr {
    if !trusted.iter().any(|network| network.contains(&peer)) {
        return peer;
    }
    let forwarded = headers
        .get_all("forwarded")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|entry| {
            entry.split(';').find_map(|part| {
                let (name, value) = part.trim().split_once('=')?;
                name.eq_ignore_ascii_case("for").then_some(value)
            })
        })
        .filter_map(parse_forwarded_ip)
        .collect::<Vec<_>>();
    let chain = if forwarded.is_empty() {
        headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
            .map(|value| {
                value
                    .split(',')
                    .filter_map(|item| item.trim().parse().ok())
                    .collect()
            })
            .unwrap_or_default()
    } else {
        forwarded
    };
    chain
        .into_iter()
        .rev()
        .find(|ip| !trusted.iter().any(|network| network.contains(ip)))
        .unwrap_or(peer)
}

fn parse_forwarded_ip(raw: &str) -> Option<IpAddr> {
    let raw = raw.trim_matches('"');
    if let Some(value) = raw.strip_prefix('[') {
        return value.split(']').next()?.parse().ok();
    }
    raw.parse()
        .ok()
        .or_else(|| raw.parse::<SocketAddr>().ok().map(|addr| addr.ip()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ignores_spoofed_forwarding_from_untrusted_peer() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "198.51.100.1".parse().unwrap());
        assert_eq!(
            canonical_client_ip("203.0.113.5".parse().unwrap(), &headers, &[]),
            "203.0.113.5".parse::<IpAddr>().unwrap()
        );
    }
    #[test]
    fn selects_first_untrusted_hop_from_right() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "198.51.100.2, 10.0.0.9".parse().unwrap());
        let trusted = vec!["10.0.0.0/8".parse().unwrap()];
        assert_eq!(
            canonical_client_ip("10.0.0.8".parse().unwrap(), &headers, &trusted),
            "198.51.100.2".parse::<IpAddr>().unwrap()
        );
    }
}

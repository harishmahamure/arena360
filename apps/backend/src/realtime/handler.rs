use axum::{
    extract::{ws::WebSocketUpgrade, State},
    http::HeaderMap,
    response::IntoResponse,
};
use std::sync::Arc;

use crate::app::AppState;
use crate::error::AppError;

use super::connection;

/// WebSocket upgrade handler at GET /realtime.
///
/// Auth is passed via `Sec-WebSocket-Protocol: bearer, <jwt>`.
/// The upgrade response echoes back `bearer` as the selected sub-protocol.
pub async fn ws_upgrade(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_ws_token(&headers)?;
    let claims = decode_token_for_ws(&state, &token)?;

    let pool = state.db.clone();
    let connections = state.ws_connections.clone();
    let outbox = state.outbox.clone();

    Ok(ws
        .protocols(["bearer"])
        .on_upgrade(move |socket| {
            connection::run(socket, claims, pool, connections, outbox)
        }))
}

fn extract_ws_token(headers: &HeaderMap) -> Result<String, AppError> {
    // Standard: Sec-WebSocket-Protocol: bearer, <token>
    let protocol_header = headers
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let parts: Vec<&str> = protocol_header.split(',').map(str::trim).collect();
    if parts.len() >= 2 && parts[0] == "bearer" {
        return Ok(parts[1].to_string());
    }

    // Fallback: Authorization header (for non-browser clients)
    if let Some(auth) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            return Ok(token.trim().to_string());
        }
    }

    Err(AppError::Unauthorized(
        "Missing authentication token. Use Sec-WebSocket-Protocol: bearer, <token>".to_string(),
    ))
}

fn decode_token_for_ws(state: &AppState, token: &str) -> Result<crate::dto::JwtUserClaims, AppError> {
    use jsonwebtoken::{decode, DecodingKey, Validation};

    let mut validation = Validation::default();
    validation.validate_exp = true;
    validation.set_audience(&["gamezone"]);
    validation.set_issuer(&["gamezone"]);

    decode::<crate::dto::JwtUserClaims>(
        token,
        &DecodingKey::from_secret(state.settings.jwt_secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|err| {
        tracing::debug!(?err, "WS JWT decode failed");
        AppError::Unauthorized("Invalid or expired token".to_string())
    })
}

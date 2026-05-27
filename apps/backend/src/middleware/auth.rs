use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use jsonwebtoken::{decode, DecodingKey, Validation};
use std::sync::Arc;

use crate::app::AppState;
use crate::dto::JwtUserClaims;
use crate::error::AppError;

const PUBLIC_EXACT: &[&str] = &[
    "/",
    "/auth/login",
    "/auth/login/admin",
    "/auth/login/staff",
    "/auth/verify-otp",
    "/health/live",
];

const PUBLIC_PREFIX: &[&str] = &["/health", "/api/docs"];

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    if req.method() == axum::http::Method::OPTIONS {
        return Ok(next.run(req).await);
    }

    let path = req.uri().path().to_string();

    if is_public(&path) {
        if let Some(token) = extract_bearer(req.headers()) {
            if let Ok(data) = decode_token(&state, token) {
                req.extensions_mut().insert(data);
            }
        }
        return Ok(next.run(req).await);
    }

    let token = extract_bearer(req.headers())
        .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))?;

    let claims = decode_token(&state, token)?;
    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}

pub fn require_admin(claims: &JwtUserClaims) -> Result<(), AppError> {
    if claims.is_admin() {
        Ok(())
    } else {
        Err(AppError::Forbidden("Admin access required".to_string()))
    }
}

pub fn require_admin_or_staff(claims: &JwtUserClaims) -> Result<(), AppError> {
    if claims.is_admin_or_staff() {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "Admin or staff access required".to_string(),
        ))
    }
}

fn is_public(path: &str) -> bool {
    if PUBLIC_EXACT.contains(&path) {
        return true;
    }
    PUBLIC_PREFIX
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")))
}

fn extract_bearer(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").or(Some(v)))
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn decode_token(state: &AppState, token: &str) -> Result<JwtUserClaims, AppError> {
    let mut validation = Validation::default();
    validation.validate_exp = true;
    validation.set_audience(&["gamezone"]);
    validation.set_issuer(&["gamezone"]);

    decode::<JwtUserClaims>(
        token,
        &DecodingKey::from_secret(state.settings.jwt_secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|err| {
        tracing::debug!(?err, "JWT decode failed");
        AppError::Unauthorized("Invalid or expired token".to_string())
    })
}

pub struct AuthUser(pub JwtUserClaims);

impl<S> axum::extract::FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<JwtUserClaims>()
            .cloned()
            .map(AuthUser)
            .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))
    }
}

pub struct AdminUser(pub JwtUserClaims);

impl<S> axum::extract::FromRequestParts<S> for AdminUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let claims = parts
            .extensions
            .get::<JwtUserClaims>()
            .cloned()
            .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))?;
        require_admin(&claims)?;
        Ok(AdminUser(claims))
    }
}

pub struct AdminOrStaff(pub JwtUserClaims);

impl<S> axum::extract::FromRequestParts<S> for AdminOrStaff
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let claims = parts
            .extensions
            .get::<JwtUserClaims>()
            .cloned()
            .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))?;
        require_admin_or_staff(&claims)?;
        Ok(AdminOrStaff(claims))
    }
}

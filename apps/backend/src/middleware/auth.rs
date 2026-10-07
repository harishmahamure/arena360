use axum::{
    body::Body,
    extract::{FromRef, State},
    http::Request,
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::JwtUserClaims;
use crate::error::AppError;

const PUBLIC_EXACT: &[&str] = &[
    "/",
    "/auth/login",
    "/auth/login/admin",
    "/auth/login/staff",
    "/auth/login/panel",
    "/auth/login/panel/mfa",
    "/branding",
    "/health/live",
    "/metrics",
    "/realtime",
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

    let mut claims = decode_token(&state, token)?;
    if claims.is_admin_or_staff() {
        let db = state.business_db(&claims).await?;
        let user = claims
            .user_id_uuid()
            .ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))?;
        let settings = crate::repositories::TenantSettingsRepository::new(db);
        let member = settings
            .membership_context(user)
            .await?
            .filter(|m| claims.roles.contains(&m.role))
            .ok_or_else(|| {
                AppError::Unauthorized("Account or tenant access changed; sign in again".into())
            })?;
        claims.permissions = settings.effective_permissions(user).await?;
        claims.roles = vec![member.role];
    }
    if claims.is_admin_or_staff() {
        crate::access::routes::authorize(&claims, req.method().as_str(), &path)?;
        let root = path.trim_matches('/').split('/').next().unwrap_or_default();
        if !matches!(
            root,
            "access"
                | "auth"
                | "branding"
                | "organizations"
                | "devices"
                | "plans"
                | "products"
                | "stats"
                | "notifications"
                | "realtime"
                | "metrics"
                | "health"
        ) && !(root == "transactions" && req.method() == axum::http::Method::POST)
            && !(root == "units" && req.method() == axum::http::Method::GET)
            && path != "/products/current-prices"
            && !matches!(path.as_str(), "/inventory/locations" | "/inventory/stock")
            && !path.starts_with("/inventory/locations/")
            && !matches!(
                path.as_str(),
                "/shifts/start"
                    | "/shifts/start-context"
                    | "/shifts/active"
                    | "/shifts/clock-out"
                    | "/shifts/close"
                    | "/shifts/handover"
            )
        {
            if let Some(permission) =
                crate::access::routes::permission(req.method().as_str(), &path, &claims.userId)
            {
                if !permission.is_empty() {
                    let user = claims
                        .user_id_uuid()
                        .ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))?;
                    let org = Uuid::parse_str(&claims.tenantId)
                        .map_err(|_| AppError::Forbidden("Select an organization".into()))?;
                    crate::repositories::TenantSettingsRepository::new(
                        state.business_db(&claims).await?,
                    )
                    .ensure_location_permission(
                        org,
                        crate::models::DEFAULT_VENUE_LOCATION_ID,
                        user,
                        &permission,
                    )
                    .await?;
                }
            }
        }
    }
    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}

/// Panel access is checked against current account and membership state, not only JWT age.
pub async fn panel_session_active(
    pool: &sqlx::PgPool,
    claims: &JwtUserClaims,
) -> Result<bool, sqlx::Error> {
    if !claims.is_admin_or_staff() {
        return Ok(true);
    }
    let Some(user_id) = claims.user_id_uuid() else {
        return Ok(false);
    };
    let Ok(organization_id) = Uuid::parse_str(&claims.tenantId) else {
        return Ok(false);
    };
    let active = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS (
            SELECT 1 FROM users u
            JOIN organization_memberships m ON m."userId" = u.id
            JOIN organizations o ON o.id=m."organizationId" AND o."isActive"
            WHERE u.id = $1 AND u."isActive" = TRUE AND u."deletedAt" IS NULL
              AND u.role = ANY($2) AND m."organizationId" = $3 AND m."isActive" = TRUE
        )"#,
    )
    .bind(user_id)
    .bind(&claims.roles)
    .bind(organization_id)
    .fetch_one(pool)
    .await?;
    if !active {
        return Ok(false);
    }
    let current = crate::access::effective(pool, organization_id, user_id).await?;
    let mut issued = claims.permissions.clone();
    issued.sort();
    issued.dedup();
    Ok(current == issued)
}

pub async fn control_panel_session_active(
    pool: &sqlx::PgPool,
    claims: &JwtUserClaims,
) -> Result<bool, sqlx::Error> {
    if !claims.is_admin_or_staff() {
        return Ok(true);
    }
    let Some(user_id) = claims.user_id_uuid() else {
        return Ok(false);
    };
    let Ok(tenant_id) = Uuid::parse_str(&claims.tenantId) else {
        return Ok(false);
    };
    let row = sqlx::query_as::<_, (serde_json::Value,)>(
        r#"SELECT m.permissions
           FROM users u
           JOIN organization_memberships m ON m.user_id = u.id
           JOIN tenants t ON t.id = m.tenant_id
             AND t.state NOT IN ('DELETED', 'FAILED')
           WHERE u.id = $1
             AND u.is_active
             AND u.deleted_at IS NULL
             AND m.tenant_id = $2
             AND m.is_active
             AND m.role = ANY($3)"#,
    )
    .bind(user_id)
    .bind(tenant_id)
    .bind(&claims.roles)
    .fetch_optional(pool)
    .await?;
    let Some((permissions,)) = row else {
        return Ok(false);
    };
    let mut current: Vec<String> = permissions
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|permission| permission.as_str().map(str::to_string))
        .collect();
    current.sort();
    current.dedup();
    let mut issued = claims.permissions.clone();
    issued.sort();
    issued.dedup();
    Ok(current == issued)
}

pub fn require_admin(claims: &JwtUserClaims) -> Result<(), AppError> {
    if crate::access::managed(claims) || claims.is_admin() {
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

pub fn require_staff(claims: &JwtUserClaims) -> Result<(), AppError> {
    if crate::access::managed(claims) && crate::access::has(claims, "shifts:write")
        || !crate::access::managed(claims) && claims.is_staff()
    {
        Ok(())
    } else {
        Err(AppError::Forbidden("Shifts are staff-only".to_string()))
    }
}

pub fn require_staff_for_counter(claims: &JwtUserClaims) -> Result<(), AppError> {
    if crate::access::managed(claims) && crate::access::has(claims, "shifts:write")
        || !crate::access::managed(claims) && claims.is_staff()
    {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "Staff login required for counter operations".to_string(),
        ))
    }
}

pub fn require_device(claims: &JwtUserClaims) -> Result<(), AppError> {
    if claims.is_device() {
        Ok(())
    } else {
        Err(AppError::Forbidden("Device access required".to_string()))
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

pub(crate) fn decode_token(state: &AppState, token: &str) -> Result<JwtUserClaims, AppError> {
    let mut validation = Validation::default();
    validation.validate_exp = true;
    validation.leeway = 0;
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

pub struct StaffUser(pub JwtUserClaims);

impl<S> axum::extract::FromRequestParts<S> for StaffUser
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
        require_staff(&claims)?;
        Ok(StaffUser(claims))
    }
}

pub struct DeviceUser(pub JwtUserClaims);

impl DeviceUser {
    pub fn device_id(&self) -> Result<uuid::Uuid, AppError> {
        require_device(&self.0)?;
        self.0
            .user_id_uuid()
            .ok_or_else(|| AppError::Internal("Invalid device ID in token".to_string()))
    }
}

impl<S> axum::extract::FromRequestParts<S> for DeviceUser
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
        require_device(&claims)?;
        Ok(DeviceUser(claims))
    }
}

pub struct PlayerUser(pub JwtUserClaims);

impl PlayerUser {
    pub fn player_id(&self) -> Result<Uuid, AppError> {
        self.0
            .user_id_uuid()
            .ok_or_else(|| AppError::Unauthorized("Invalid player ID in token".to_string()))
    }

    pub fn device_id(&self) -> Result<Uuid, AppError> {
        self.0
            .device_id_uuid()
            .ok_or_else(|| AppError::Unauthorized("Player token missing deviceId".to_string()))
    }
}

impl<S> axum::extract::FromRequestParts<S> for PlayerUser
where
    S: Send + Sync,
    Arc<AppState>: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let app_state = Arc::<AppState>::from_ref(state);

        let device_claims = parts
            .extensions
            .get::<JwtUserClaims>()
            .cloned()
            .ok_or_else(|| AppError::Unauthorized("Device authentication required".to_string()))?;
        require_device(&device_claims)?;

        let player_token = parts
            .headers
            .get("X-Player-Token")
            .or_else(|| parts.headers.get("x-player-token"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer ").or(Some(v)))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                AppError::Unauthorized("X-Player-Token required for player routes".to_string())
            })?;

        let player_claims = decode_token(&app_state, player_token)?;

        if !player_claims
            .roles
            .iter()
            .any(|r| r == "player" || r == "staff")
        {
            return Err(AppError::Forbidden(
                "Player or staff access required".to_string(),
            ));
        }

        let player_device = player_claims
            .device_id_uuid()
            .ok_or_else(|| AppError::Unauthorized("Player token missing deviceId".to_string()))?;
        let kiosk_device = device_claims
            .user_id_uuid()
            .ok_or_else(|| AppError::Internal("Invalid device ID in token".to_string()))?;

        if player_device != kiosk_device
            || player_claims.tenantId != device_claims.tenantId
            || player_claims.locationId != device_claims.locationId
        {
            return Err(AppError::Forbidden(
                "Player token deviceId does not match device token".to_string(),
            ));
        }

        Ok(PlayerUser(player_claims))
    }
}

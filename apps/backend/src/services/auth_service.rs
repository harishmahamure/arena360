use std::sync::Arc;

use bcrypt::verify;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::Settings;
use crate::dto::{
    ActiveSessionDto, AuthResponseDto, JwtUserClaims, LoginDto, PanelLoginResponseDto, PanelMfaDto,
    RateLimitClaims, StaffLoginDto,
};
use crate::error::AppError;
use crate::models::{
    deduction_profile::DeductionProfile, Device, PlayerPlanBalance, User,
};
use crate::repositories::{
    TenantBalanceRepository, TenantSessionRepository,
};
use crate::services::session_service::display_remaining_for_session;
use crate::services::totp_util::verify_totp_code;
use crate::services::BalanceService;
use crate::tenancy::TenantDb;
use crate::validation::{normalize_username, trim_secret};

const TENANT_PLAYER_DUMMY_PASSWORD_HASH: &str =
    "$2b$12$dprJEXAvjHcojSitMeEB1uwqnBOMCoctwdsvqEIhYFRxP1IGRvmx6";

pub struct AuthService {
    control_pool: Option<PgPool>,
    settings: Arc<Settings>,
}

impl AuthService {
    pub fn new(settings: Arc<Settings>) -> Self {
        Self {
            control_pool: None,
            settings,
        }
    }

    pub fn with_control_pool(mut self, control_pool: Option<PgPool>) -> Self {
        self.control_pool = control_pool;
        self
    }

    fn control_database(&self) -> Result<&PgPool, AppError> {
        self.control_pool.as_ref().ok_or_else(|| AppError::Api {
            code: "CONTROL_AUTH_UNAVAILABLE".into(),
            status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
            details: None,
        })
    }

    pub async fn login_admin(&self, dto: StaffLoginDto) -> Result<AuthResponseDto, AppError> {
        let username = normalize_username(&dto.username);
        let password = trim_secret(&dto.password);
        let user = self.authenticate_admin(&username, &password).await?;
        Self::verify_totp_if_enabled(&user, dto.totp.as_deref())?;
        self.issue_auth_response(&user).await
    }

    pub async fn login_staff(&self, dto: StaffLoginDto) -> Result<AuthResponseDto, AppError> {
        let username = normalize_username(&dto.username);
        let password = trim_secret(&dto.password);
        let user = match self.authenticate_staff(&username, &password).await {
            Ok(u) => u,
            Err(e) => {
                tracing::error!("authenticate_staff failed: {:?}", e);
                return Err(e);
            }
        };

        Self::verify_totp_if_enabled(&user, dto.totp.as_deref())?;

        let token = self.generate_access_token(&user).await?;
        Ok(AuthResponseDto {
            accessToken: token,
            user: user.to_auth_user(),
            shiftId: None,
            activeSession: None,
        })
    }

    pub async fn login_panel(&self, dto: StaffLoginDto) -> Result<PanelLoginResponseDto, AppError> {
        self.control_database()?;
        let user = self
            .authenticate_panel(
                &normalize_username(&dto.username),
                &trim_secret(&dto.password),
            )
            .await
            .map_err(|_| AppError::unauthorized_code("AUTH_INVALID_CREDENTIALS"))?;

        if user.totp_enabled {
            let now = Utc::now();
            let expires_at = now + Duration::minutes(5);
            let challenge_token = format!(
                "pch_{}_{}",
                Uuid::new_v4().simple(),
                Uuid::new_v4().simple()
            );
            let token_hash = Self::panel_challenge_hash(&challenge_token);
            let pool = self.control_database()?;
            sqlx::query(
                r#"INSERT INTO auth_challenges
                        (token_hash, user_id, kind, expires_at, attempts, created_at)
                       VALUES ($1, $2, 'PANEL_MFA', $3, 0, NOW())
                       ON CONFLICT (user_id, kind) DO UPDATE SET
                         token_hash = EXCLUDED.token_hash,
                         expires_at = EXCLUDED.expires_at,
                         attempts = 0,
                         created_at = NOW()"#,
            )
            .bind(token_hash)
            .bind(user.id)
            .bind(expires_at)
            .execute(pool)
            .await?;
            return Ok(PanelLoginResponseDto::MfaRequired {
                challenge_token,
                expires_at,
            });
        }

        self.panel_authenticated_response(&user).await
    }

    pub async fn verify_panel_mfa(
        &self,
        dto: PanelMfaDto,
    ) -> Result<PanelLoginResponseDto, AppError> {
        let token_hash = Self::panel_challenge_hash(dto.challengeToken.trim());
        let pool = self.control_database()?;
        let mut tx = pool.begin().await?;
        let challenge: Option<(Uuid, chrono::DateTime<Utc>, i32)> = sqlx::query_as(
            r#"SELECT user_id, expires_at, attempts
                       FROM auth_challenges
                       WHERE token_hash = $1 AND kind = 'PANEL_MFA'
                       FOR UPDATE"#,
        )
        .bind(&token_hash)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((user_id, expires_at, attempts)) = challenge else {
            return Err(AppError::unauthorized_code("AUTH_CHALLENGE_EXPIRED"));
        };
        if expires_at <= Utc::now() || attempts >= 5 {
            let delete_sql =
                "DELETE FROM auth_challenges WHERE token_hash = $1 AND kind = 'PANEL_MFA'";
            sqlx::query(delete_sql)
                .bind(&token_hash)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            return Err(AppError::unauthorized_code("AUTH_CHALLENGE_EXPIRED"));
        }
        let user = self
            .identity_user_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::unauthorized_code("AUTH_CHALLENGE_EXPIRED"))
            .map_err(|_| AppError::unauthorized_code("AUTH_CHALLENGE_EXPIRED"))?;

        if !matches!(user.role.as_deref(), Some("admin" | "staff")) || !user.is_active {
            return Err(AppError::unauthorized_code("AUTH_INVALID_CREDENTIALS"));
        }
        let secret = user
            .totp_secret
            .as_deref()
            .ok_or_else(|| AppError::unauthorized_code("AUTH_INVALID_MFA"))?;
        if !verify_totp_code(secret, dto.code.trim(), &user.username)? {
            if attempts >= 4 {
                let delete_sql =
                    "DELETE FROM auth_challenges WHERE token_hash = $1 AND kind = 'PANEL_MFA'";
                sqlx::query(delete_sql)
                    .bind(&token_hash)
                    .execute(&mut *tx)
                    .await?;
            } else {
                let update_sql = "UPDATE auth_challenges SET attempts = attempts + 1 \
                     WHERE token_hash = $1 AND kind = 'PANEL_MFA'";
                sqlx::query(update_sql)
                    .bind(&token_hash)
                    .execute(&mut *tx)
                    .await?;
            }
            tx.commit().await?;
            return Err(AppError::unauthorized_code("AUTH_INVALID_MFA"));
        }

        let delete_sql = "DELETE FROM auth_challenges WHERE token_hash = $1 AND kind = 'PANEL_MFA'";
        sqlx::query(delete_sql)
            .bind(&token_hash)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        self.panel_authenticated_response(&user).await
    }

    fn panel_challenge_hash(token: &str) -> String {
        hex::encode(Sha256::digest(token.as_bytes()))
    }

    async fn authenticate_panel(&self, username: &str, password: &str) -> Result<User, AppError> {
        let user = self
            .identity_user_by_username(username, None)
            .await?
            .ok_or_else(|| AppError::unauthorized_code("AUTH_INVALID_CREDENTIALS"))?;
        if !matches!(user.role.as_deref(), Some("admin" | "staff")) {
            return Err(AppError::unauthorized_code("AUTH_INVALID_CREDENTIALS"));
        }
        self.verify_password(password, user.password_hash.as_deref())?;
        self.ensure_active(&user)?;
        Ok(user)
    }

    async fn panel_authenticated_response(
        &self,
        user: &User,
    ) -> Result<PanelLoginResponseDto, AppError> {
        let token = self.generate_access_token(user).await?;
        let pool = self.control_database()?;
        let permissions: Vec<String> = sqlx::query_scalar::<_, serde_json::Value>(
            r#"SELECT m.permissions
                   FROM organization_memberships m
                   JOIN tenants t ON t.id = m.tenant_id
                   WHERE m.user_id = $1 AND m.role = $2 AND m.is_active
                     AND t.state NOT IN ('DELETED', 'FAILED')
                   ORDER BY m.created_at
                   LIMIT 1"#,
        )
        .bind(user.id)
        .bind(user.role.as_deref())
        .fetch_one(pool)
        .await?
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|permission| permission.as_str().map(str::to_string))
        .collect();
        let next_step = if permissions.iter().any(|p| p == "shifts:write")
            && !permissions.iter().any(|p| p == "access:manage")
        {
            "shift_setup"
        } else {
            "dashboard"
        };
        Ok(PanelLoginResponseDto::Authenticated {
            access_token: token,
            user: user.to_auth_user(),
            next_step: next_step.to_string(),
        })
    }

    pub async fn login_player_tenant(
        &self,
        db: Arc<TenantDb>,
        device: &Device,
        dto: LoginDto,
        timezone: String,
    ) -> Result<AuthResponseDto, AppError> {
        if device.organization_id != db.tenant_id() {
            return Err(AppError::Unauthorized("Kiosk tenant mismatch".into()));
        }
        if device.registration_status != "registered" {
            return Err(AppError::forbidden_code("DEVICE_NOT_REGISTERED"));
        }
        if device.status == "under_maintenance" {
            return Err(AppError::forbidden_code("DEVICE_UNDER_MAINTENANCE"));
        }
        let username = normalize_username(&dto.username);
        let local_player = crate::repositories::TenantUserRepository::new(db.clone()).find_player_for_auth(&username).await?;
        let global_staff = if local_player.is_none() {
            if let Some(control) = &self.control_pool {
                crate::control::identity::IdentityRepository::new(control.clone()).kiosk_staff_identity(db.tenant_id(), &username).await?
            } else { None }
        } else { None };
        let authenticating_staff = global_staff.is_some();
        let user = local_player.or(global_staff);
        let password = trim_secret(&dto.password);
        let password_hash = user
            .as_ref()
            .and_then(|found| found.password_hash.as_deref())
            .unwrap_or(TENANT_PLAYER_DUMMY_PASSWORD_HASH);
        let password_valid = verify(&password, password_hash).unwrap_or(false);
        let user = match (user, password_valid) {
            (Some(user), true) => user,
            _ => return Err(AppError::unauthorized_code("AUTH_INVALID_CREDENTIALS")),
        };
        let user = if authenticating_staff {
            crate::control::staff_projection::sync_user(self.control_pool.as_ref().unwrap(), db.clone(), user.id).await?;
            let projected = crate::repositories::TenantUserRepository::new(db.clone()).require_active_staff(user.id).await?;
            if projected.role.as_deref() != Some("staff") {return Err(AppError::unauthorized_code("AUTH_INVALID_CREDENTIALS"));}
            projected
        } else { user };
        let is_staff = user.role.as_deref() == Some("staff");
        if is_staff {
            if crate::repositories::TenantShiftRepository::new(db.clone()).find_active_by_user(user.id).await?.is_some() {
                return Err(AppError::forbidden_code("STAFF_SHIFT_ACTIVE"));
            }
            let allowance = TenantBalanceRepository::new(db.clone()).find_existing_for_scope(user.id,None,None,crate::models::plan_kind::STAFF_ALLOWANCE).await?
                .ok_or_else(||AppError::forbidden_code("STAFF_ALLOWANCE_NONE"))?;
            let validation = BalanceService::validate_balance(&allowance,Some(device),None);
            if !validation.valid { return Err(BalanceService::validation_to_app_error_for_balance(&allowance,validation)); }
        }
        let sessions = TenantSessionRepository::new(db.clone());
        let active_session = if let Some(open) = sessions.find_open_for_player(user.id).await? {
            if open.device_id != device.id {
                return Err(AppError::conflict_code(
                    "PLAYER_ALREADY_IN_SESSION",
                    Some(
                        json!({"deviceId":open.device_id,"deviceName":open.device_name,
                        "sessionId":open.session_id,"sessionStartTime":crate::time::utc_timestamp(&open.start_time)}),
                    ),
                ));
            }
            let balance = TenantBalanceRepository::new(db.clone())
                .find_by_id(open.balance_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Open balance record missing".into()))?;
            let validation = BalanceService::validate_balance(&balance, Some(device), None);
            if !validation.valid {
                return Err(BalanceService::validation_to_app_error_for_balance(
                    &balance, validation,
                ));
            }
            let session = sessions
                .find_by_id(open.session_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Open session record missing".into()))?;
            Some(ActiveSessionDto {
                id: session.id.to_string(),
                startTime: session.start_time,
                balanceId: balance.id.to_string(),
                remainingMinutes: display_remaining_for_session(&balance, &session, &timezone)
                    as f64,
                walletBalanceMinutes: balance.remaining_minutes as f64,
                deductionProfile: crate::services::session_service::session_profile_value(
                    &balance, &session,
                )
                .and_then(|value| serde_json::from_value::<DeductionProfile>(value.clone()).ok()),
                cafeTimezone: timezone,
                timeCreditsConsumed: Some(session.time_credits_consumed.unwrap_or(0) as f64),
                expiryDate: balance.expiry_date,
            })
        } else if is_staff {
            None
        } else {
            let balances = TenantBalanceRepository::new(db)
                .list(&crate::models::BalanceFilterDto {
                    player_id: Some(user.id),
                    status: Some("active".into()),
                    usable_only: Some(true),
                    limit: Some(100),
                    ..Default::default()
                })
                .await?;
            let usable = balances.data.into_iter().any(|row| {
                let raw: PlayerPlanBalance = PlayerPlanBalance {
                    id: row.id,
                    player_id: row.player_id,
                    device_type: row.device_type,
                    device_sub_type: row.device_sub_type,
                    kind: row.kind,
                    remaining_minutes: row.remaining_minutes,
                    expiry_date: row.expiry_date,
                    window_start: row.window_start,
                    window_end: row.window_end,
                    status: row.status,
                    source_plan_id: row.source_plan_id,
                    allowed_days: row.allowed_days,
                    allowed_months: row.allowed_months,
                    deduction_profile: row.deduction_profile,
                    created_by: row.created_by,
                    updated_by: row.updated_by,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    deleted_at: row.deleted_at,
                };
                BalanceService::device_scope_matches(&raw, device)
                    && BalanceService::validate_balance(&raw, Some(device), None).valid
            });
            if !usable {
                return Err(AppError::forbidden_code("PLAN_NOT_ACTIVATED"));
            }
            None
        };
        let token = self.generate_player_token_for(
            &user,
            device.id,
            device.organization_id,
            device.location_id,
        )?;
        Ok(AuthResponseDto {
            accessToken: token,
            user: user.to_auth_user(),
            shiftId: None,
            activeSession: active_session,
        })
    }

    pub fn generate_device_token_for(
        &self,
        device_id: Uuid,
        tenant_id: Uuid,
        location_id: Uuid,
    ) -> Result<String, AppError> {
        let now = Utc::now();
        let exp_duration = parse_duration(&self.settings.jwt_device_expiration);
        let id = device_id.to_string();
        let tenant_id = tenant_id.to_string();

        let claims = JwtUserClaims {
            sub: id.clone(),
            permissions: vec![],
            allowedTenants: vec![tenant_id.clone()],
            rateLimit: Some(RateLimitClaims { qps: 100 }),
            iss: "gamezone".to_string(),
            aud: serde_json::json!("gamezone"),
            iat: Some(now.timestamp()),
            exp: Some((now + exp_duration).timestamp()),
            userId: id.clone(),
            tenantId: tenant_id.clone(),
            roles: vec!["device".to_string()],
            appId: "game-zone-kiosk".to_string(),
            orgIds: vec![tenant_id],
            deviceId: Some(id),
            locationId: Some(location_id.to_string()),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.settings.jwt_secret.as_bytes()),
        )
        .map_err(AppError::Jwt)
    }

    pub fn generate_player_token_for(
        &self,
        user: &User,
        device_id: Uuid,
        tenant_id: Uuid,
        location_id: Uuid,
    ) -> Result<String, AppError> {
        let now = Utc::now();
        let exp_duration = parse_duration(&self.settings.jwt_player_expiration);
        // Playing credentials carry player capabilities even when the account is staff.
        let role = "player".to_string();
        let tenant_id = tenant_id.to_string();

        let claims = JwtUserClaims {
            sub: user.id.to_string(),
            permissions: vec![],
            allowedTenants: vec![tenant_id.clone()],
            rateLimit: Some(RateLimitClaims { qps: 100 }),
            iss: "gamezone".to_string(),
            aud: serde_json::json!("gamezone"),
            iat: Some(now.timestamp()),
            exp: Some((now + exp_duration).timestamp()),
            userId: user.id.to_string(),
            tenantId: tenant_id.clone(),
            roles: vec![role],
            appId: "game-zone-kiosk".to_string(),
            orgIds: vec![tenant_id],
            deviceId: Some(device_id.to_string()),
            locationId: Some(location_id.to_string()),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.settings.jwt_secret.as_bytes()),
        )
        .map_err(AppError::Jwt)
    }

    pub async fn authenticate_staff(
        &self,
        username: &str,
        password: &str,
    ) -> Result<User, AppError> {
        let user = self
            .identity_user_by_username(username, Some("staff"))
            .await?
            .ok_or_else(|| {
                tracing::error!(
                    "authenticate_staff failed: User not found for username: {}",
                    username
                );
                AppError::Unauthorized("User not found".to_string())
            })?;

        if user.role.as_deref() != Some("staff") {
            tracing::error!(
                "authenticate_staff failed: User {} is not staff (role: {:?})",
                username,
                user.role
            );
            return Err(AppError::Unauthorized("User is not staff".to_string()));
        }

        if let Err(e) = self.verify_password(password, user.password_hash.as_deref()) {
            tracing::error!(
                "authenticate_staff failed: Invalid password for user {}",
                username
            );
            return Err(e);
        }

        if let Err(e) = self.ensure_active(&user) {
            tracing::error!("authenticate_staff failed: User {} is not active", username);
            return Err(e);
        }

        Ok(user)
    }

    async fn identity_user_by_username(
        &self,
        username: &str,
        required_role: Option<&str>,
    ) -> Result<Option<User>, AppError> {
        let pool = self.control_database()?;

        sqlx::query_as::<_, User>(
            r#"SELECT u.id, u.email, u.username, u.password_hash, u.is_active,
                      u.first_name, u.last_name, u.phone_number, membership.role,
                      0::float8 AS credit_limit,
                      NULL::text AS session_otp_id, NULL::text AS session_otp,
                      u.totp_secret, u.totp_enabled,
                      NULL::uuid AS created_by, NULL::uuid AS updated_by,
                      u.created_at, u.updated_at, u.deleted_at, u.avatar_url
               FROM users u
               JOIN LATERAL (
                 SELECT m.role
                 FROM organization_memberships m
                 JOIN tenants t ON t.id = m.tenant_id
                 WHERE m.user_id = u.id
                   AND m.is_active
                   AND t.state NOT IN ('DELETED', 'FAILED')
                   AND ($2::text IS NULL OR m.role = $2)
                 ORDER BY m.created_at
                 LIMIT 1
               ) membership ON TRUE
               WHERE u.username = $1 AND u.deleted_at IS NULL"#,
        )
        .bind(username)
        .bind(required_role)
        .fetch_optional(pool)
        .await
        .map_err(AppError::Database)
    }

    async fn identity_user_by_id(&self, user_id: Uuid) -> Result<Option<User>, AppError> {
        let pool = self.control_database()?;

        sqlx::query_as::<_, User>(
            r#"SELECT u.id, u.email, u.username, u.password_hash, u.is_active,
                      u.first_name, u.last_name, u.phone_number, membership.role,
                      0::float8 AS credit_limit,
                      NULL::text AS session_otp_id, NULL::text AS session_otp,
                      u.totp_secret, u.totp_enabled,
                      NULL::uuid AS created_by, NULL::uuid AS updated_by,
                      u.created_at, u.updated_at, u.deleted_at, u.avatar_url
               FROM users u
               JOIN LATERAL (
                 SELECT m.role
                 FROM organization_memberships m
                 JOIN tenants t ON t.id = m.tenant_id
                 WHERE m.user_id = u.id AND m.is_active
                   AND t.state NOT IN ('DELETED', 'FAILED')
                 ORDER BY m.created_at
                 LIMIT 1
               ) membership ON TRUE
               WHERE u.id = $1 AND u.deleted_at IS NULL"#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::Database)
    }

    pub async fn verify_optional_staff_totp(&self, user_id: Uuid, code: Option<&str>) -> Result<(), AppError> {
        let user = self.identity_user_by_id(user_id).await?
            .ok_or_else(|| AppError::Unauthorized("Staff identity not found".into()))?;
        self.ensure_active(&user)?;
        Self::verify_totp_if_enabled(&user, code)
    }

    pub async fn authenticate_staff_with_totp(
        &self,
        username: &str,
        password: &str,
        totp: &str,
    ) -> Result<User, AppError> {
        let user = self.authenticate_staff(username, password).await?;

        if !user.totp_enabled {
            return Err(AppError::BadRequest(
                "Validator does not have TOTP enabled".to_string(),
            ));
        }

        let Some(secret) = user.totp_secret.as_deref() else {
            return Err(AppError::BadRequest(
                "Validator does not have TOTP configured".to_string(),
            ));
        };

        if verify_totp_code(secret, totp, &user.username)? {
            Ok(user)
        } else {
            Err(AppError::Unauthorized("Invalid TOTP code".to_string()))
        }
    }

    /// Authentication can consult the control plane; the issued grants come from
    /// the selected tenant. Refresh and handover keep this tenant selected.
    pub async fn issue_tenant_auth_response(&self, db: Arc<crate::tenancy::TenantDb>, user_id: Uuid) -> Result<AuthResponseDto, AppError> {
        let pool = self.control_pool.as_ref().ok_or_else(|| AppError::Api { code:"CONTROL_AUTH_UNAVAILABLE".into(),status:axum::http::StatusCode::SERVICE_UNAVAILABLE,details:None })?;
        let memberships: Vec<(Uuid,String)> = sqlx::query_as("SELECT m.tenant_id,m.role FROM organization_memberships m JOIN users u ON u.id=m.user_id JOIN tenants t ON t.id=m.tenant_id WHERE m.user_id=$1 AND m.is_active AND u.is_active AND u.deleted_at IS NULL AND t.is_enabled AND EXISTS(SELECT 1 FROM subscriptions s WHERE s.tenant_id=t.id AND s.status IN ('TRIAL','ACTIVE') AND s.ends_at>clock_timestamp()) AND t.state NOT IN('DELETED','FAILED') ORDER BY m.created_at,m.tenant_id")
            .bind(user_id).fetch_all(pool).await?;
        let role = memberships.iter().find(|m| m.0 == db.tenant_id()).map(|m| m.1.clone())
            .ok_or_else(|| AppError::Forbidden("Active membership required in the selected tenant".into()))?;
        crate::control::staff_projection::sync_user(pool,db.clone(),user_id).await?;
        let mut user = crate::repositories::TenantUserRepository::new(db.clone()).require_active_staff(user_id).await?;
        user.role = Some(role);
        let grants = crate::repositories::TenantSettingsRepository::new(db.clone()).effective_permissions(user_id).await?;
        let mut scopes = vec![(db.tenant_id(),serde_json::json!(grants))];
        scopes.extend(memberships.into_iter().filter(|m| m.0 != db.tenant_id()).map(|m| (m.0,serde_json::json!([]))));
        Ok(AuthResponseDto { accessToken:self.encode_access_token(&user,&scopes)?,user:user.to_auth_user(),shiftId:None,activeSession:None })
    }

    /// Renew a valid owning-cell session from the local membership projection.
    /// Allowed tenant IDs remain bounded by the previously signed token.
    pub async fn issue_local_tenant_auth_response(&self,db:Arc<crate::tenancy::TenantDb>,user_id:Uuid,allowed:&[String]) -> Result<AuthResponseDto,AppError> {
        if !allowed.iter().any(|id| id==&db.tenant_id().to_string()) {
            return Err(AppError::Forbidden("Token does not allow the selected tenant".into()));
        }
        let user=crate::repositories::TenantUserRepository::new(db.clone()).require_active_staff(user_id).await?;
        let grants=crate::repositories::TenantSettingsRepository::new(db.clone()).effective_permissions(user_id).await?;
        let mut scopes=vec![(db.tenant_id(),serde_json::json!(grants))];
        let mut seen=std::collections::HashSet::from([db.tenant_id()]);
        for id in allowed.iter().filter_map(|id|Uuid::parse_str(id).ok()) {
            if seen.insert(id) {scopes.push((id,serde_json::json!([])));}
        }
        Ok(AuthResponseDto {accessToken:self.encode_access_token(&user,&scopes)?,user:user.to_auth_user(),shiftId:None,activeSession:None})
    }

    pub async fn issue_auth_response(&self, user: &User) -> Result<AuthResponseDto, AppError> {
        let token = self.generate_access_token(user).await?;
        Ok(AuthResponseDto {
            accessToken: token,
            user: user.to_auth_user(),
            shiftId: None,
            activeSession: None,
        })
    }

    pub async fn authenticate_admin(
        &self,
        username: &str,
        password: &str,
    ) -> Result<User, AppError> {
        let user = self
            .identity_user_by_username(username, Some("admin"))
            .await?
            .ok_or_else(|| AppError::Unauthorized("User not found".to_string()))?;

        if user.role.as_deref() != Some("admin") {
            return Err(AppError::Unauthorized("User is not an admin".to_string()));
        }

        self.verify_password(password, user.password_hash.as_deref())?;
        self.ensure_active(&user)?;

        Ok(user)
    }

    fn verify_totp_if_enabled(user: &User, totp: Option<&str>) -> Result<(), AppError> {
        if !user.totp_enabled {
            return Ok(());
        }

        let totp_code = match totp {
            Some(code) if !code.trim().is_empty() => code.trim(),
            _ => {
                tracing::error!("TOTP code is required but not provided or empty");
                return Err(AppError::BadRequest("TOTP code is required".to_string()));
            }
        };

        let secret = user.totp_secret.as_deref().ok_or_else(|| {
            tracing::error!("User does not have TOTP configured");
            AppError::BadRequest("User does not have TOTP configured".to_string())
        })?;

        if !verify_totp_code(secret, totp_code, &user.username)? {
            tracing::error!("Invalid TOTP code");
            return Err(AppError::Unauthorized("Invalid TOTP code".to_string()));
        }

        Ok(())
    }

    fn verify_password(&self, password: &str, hash: Option<&str>) -> Result<(), AppError> {
        let Some(hash) = hash else {
            return Err(AppError::Unauthorized("Invalid credentials".to_string()));
        };
        if verify(password, hash).unwrap_or(false) {
            Ok(())
        } else {
            Err(AppError::Unauthorized("Invalid credentials".to_string()))
        }
    }

    fn ensure_active(&self, user: &User) -> Result<(), AppError> {
        if user.is_active {
            Ok(())
        } else {
            Err(AppError::Unauthorized("User is not active".to_string()))
        }
    }

    async fn generate_access_token(&self, user: &User) -> Result<String, AppError> {
        let pool = self.control_database()?;
        let memberships: Vec<(Uuid, serde_json::Value)> = sqlx::query_as(
            r#"SELECT m.tenant_id, m.permissions
                       FROM organization_memberships m
                       JOIN tenants t ON t.id = m.tenant_id
                       WHERE m.user_id = $1 AND m.is_active
                         AND t.state NOT IN ('DELETED', 'FAILED')
                       ORDER BY (m.role = $2) DESC, m.created_at, m.tenant_id"#,
        )
        .bind(user.id)
        .bind(user.role.as_deref())
        .fetch_all(pool)
        .await?;
        if memberships.is_empty() {
            return Err(AppError::Forbidden(
                "User has no active organization membership".to_string(),
            ));
        }
        self.encode_access_token(user, &memberships)
    }

    fn encode_access_token(
        &self,
        user: &User,
        memberships: &[(Uuid, serde_json::Value)],
    ) -> Result<String, AppError> {
        let role = user.role.clone().unwrap_or_else(|| "player".to_string());
        let now = Utc::now();
        let exp_duration = parse_duration(&self.settings.jwt_access_expiration);
        let org_ids: Vec<String> = memberships.iter().map(|(id, _)| id.to_string()).collect();
        let permissions: Vec<String> = memberships
            .iter()
            .take(1)
            .flat_map(|(_, value)| {
                value
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|permission| permission.as_str().map(str::to_string))
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let tenant_id = org_ids
            .first()
            .cloned()
            .ok_or_else(|| AppError::Forbidden("No active organization selected".to_string()))?;

        let claims = JwtUserClaims {
            sub: user.id.to_string(),
            permissions,
            allowedTenants: org_ids.clone(),
            rateLimit: Some(RateLimitClaims { qps: 100 }),
            iss: "gamezone".to_string(),
            aud: serde_json::json!("gamezone"),
            iat: Some(now.timestamp()),
            exp: Some((now + exp_duration).timestamp()),
            userId: user.id.to_string(),
            tenantId: tenant_id,
            roles: vec![role],
            appId: "game-zone-backend".to_string(),
            orgIds: org_ids,
            deviceId: None,
            locationId: None,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.settings.jwt_secret.as_bytes()),
        )
        .map_err(AppError::Jwt)
    }
}

fn parse_duration(value: &str) -> Duration {
    if value.ends_with('m') {
        let mins: i64 = value.trim_end_matches('m').parse().unwrap_or(15);
        Duration::minutes(mins)
    } else if value.ends_with('d') {
        let days: i64 = value.trim_end_matches('d').parse().unwrap_or(7);
        Duration::days(days)
    } else if value.ends_with('h') {
        let hours: i64 = value.trim_end_matches('h').parse().unwrap_or(1);
        Duration::hours(hours)
    } else {
        Duration::days(7)
    }
}

#[cfg(test)]
mod admin_totp_tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    use crate::services::totp_util::generate_totp_setup;

    fn test_user(username: &str, totp_enabled: bool, totp_secret: Option<String>) -> User {
        let now = Utc::now();
        User {
            id: Uuid::new_v4(),
            email: None,
            username: username.to_string(),
            password_hash: None,
            is_active: true,
            first_name: None,
            last_name: None,
            phone_number: None,
            role: Some("admin".to_string()),
            credit_limit: 0.0,
            session_otp_id: None,
            session_otp: None,
            totp_secret,
            totp_enabled,
            created_by: None,
            updated_by: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            avatar_url: None,
        }
    }

    #[test]
    fn verify_totp_if_enabled_skips_when_disabled() {
        let user = test_user("admin1", false, None);
        AuthService::verify_totp_if_enabled(&user, None).expect("password-only admin");
    }

    #[test]
    fn verify_totp_if_enabled_requires_code_when_enabled() {
        let user = test_user("admin1", true, Some("JBSWY3DPEHPK3PXP".to_string()));
        let err = AuthService::verify_totp_if_enabled(&user, None).unwrap_err();
        assert!(matches!(err, AppError::BadRequest(ref msg) if msg == "TOTP code is required"));
    }

    #[test]
    fn verify_totp_if_enabled_rejects_invalid_code() {
        let (secret, _) = generate_totp_setup("admin").expect("setup");
        let user = test_user("admin", true, Some(secret));
        let err = AuthService::verify_totp_if_enabled(&user, Some("000000")).unwrap_err();
        assert!(matches!(err, AppError::Unauthorized(ref msg) if msg == "Invalid TOTP code"));
    }

    #[test]
    fn verify_totp_if_enabled_accepts_valid_code() {
        use totp_rs::{Algorithm, Secret, TOTP};

        let (secret, _) = generate_totp_setup("admin").expect("setup");
        let user = test_user("admin", true, Some(secret.clone()));

        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            Secret::Encoded(secret).to_bytes().expect("secret bytes"),
            Some("GameZone".to_string()),
            "admin".to_string(),
        )
        .expect("totp");
        let code = totp.generate_current().expect("current code");

        AuthService::verify_totp_if_enabled(&user, Some(&code)).expect("valid admin totp");
    }
}

#[cfg(test)]
mod access_token_tests {
    use super::*;
    use jsonwebtoken::{decode, DecodingKey, Validation};
    use std::sync::Arc;

    use crate::config::Settings;

    fn test_settings(jwt_access_expiration: &str) -> Arc<Settings> {
        Arc::new(Settings {
            roles: crate::config::Roles::ALL,
            cell_id: None,
            tenant_data_dir: std::path::PathBuf::from("data/tenants"),
            control_database_url: None,
            database_min_connections: 0,
            database_max_connections: 1,
            database_acquire_timeout_seconds: 2,
            database_idle_timeout_seconds: 600,
            database_max_lifetime_seconds: 1800,
            nats_url: None,
            redis_url: None,
            jwt_secret: "your-jwt-secret-change-this-my-secret-sova".to_string(),
            jwt_access_expiration: jwt_access_expiration.to_string(),
            jwt_player_expiration: "24h".to_string(),
            jwt_device_expiration: "365d".to_string(),
            bcrypt_salt_rounds: 10,
            port: 3000,
            cafe_timezone: "UTC".to_string(),
            zeptomail_token: None,
            legacy_rest_enabled: false,
            trusted_proxy_cidrs: Vec::new(),
            max_concurrent_requests: 256,
        })
    }

    fn test_auth_service(settings: Arc<Settings>) -> AuthService {
        AuthService::new(settings)
    }

    #[tokio::test]
    async fn panel_auth_requires_control_storage_without_a_legacy_fallback() {
        let auth = test_auth_service(test_settings("1h"));
        let error = auth.login_panel(StaffLoginDto {
            username: "staff".into(), password: "password".into(), totp: None,
        }).await.unwrap_err();
        assert!(matches!(error, AppError::Api { ref code, status, .. }
            if code == "CONTROL_AUTH_UNAVAILABLE" && status == axum::http::StatusCode::SERVICE_UNAVAILABLE));
        let error = auth.verify_panel_mfa(PanelMfaDto {
            challengeToken: "missing".into(), code: "123456".into(),
        }).await.unwrap_err();
        assert!(matches!(error, AppError::Api { ref code, .. }
            if code == "CONTROL_AUTH_UNAVAILABLE"));
    }

    fn test_user() -> User {
        let now = Utc::now();
        User {
            id: Uuid::new_v4(),
            email: None,
            username: "admin".to_string(),
            password_hash: None,
            is_active: true,
            first_name: None,
            last_name: None,
            phone_number: None,
            role: Some("admin".to_string()),
            credit_limit: 0.0,
            session_otp_id: None,
            session_otp: None,
            totp_secret: None,
            totp_enabled: false,
            created_by: None,
            updated_by: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            avatar_url: None,
        }
    }

    #[test]
    fn panel_challenge_hash_is_deterministic_and_does_not_store_the_token() {
        let token = "pch_private-single-use-token";
        let hash = AuthService::panel_challenge_hash(token);
        assert_eq!(hash.len(), 64);
        assert_ne!(hash, token);
        assert_eq!(hash, AuthService::panel_challenge_hash(token));
        assert_ne!(hash, AuthService::panel_challenge_hash("pch_other-token"));
    }

    #[tokio::test]
    async fn admin_token_ttl_follows_jwt_access_expiration_setting() {
        let settings = test_settings("7d");
        let auth = test_auth_service(settings);
        let user = test_user();

        let token = auth
            .encode_access_token(
                &user,
                &[(
                    Uuid::now_v7(),
                    serde_json::json!(["settings:read"]),
                )],
            )
            .expect("token");
        let mut validation = Validation::default();
        validation.validate_exp = false;
        validation.set_audience(&["gamezone"]);
        validation.set_issuer(&["gamezone"]);

        let claims = decode::<JwtUserClaims>(
            &token,
            &DecodingKey::from_secret("your-jwt-secret-change-this-my-secret-sova".as_bytes()),
            &validation,
        )
        .expect("decode")
        .claims;

        let iat = claims.iat.expect("iat");
        let exp = claims.exp.expect("exp");
        let ttl_secs = exp - iat;
        let seven_days_secs = 7 * 24 * 60 * 60;
        assert!(
            ttl_secs >= seven_days_secs - 60,
            "expected ~7d TTL, got {ttl_secs}s"
        );
        assert!(ttl_secs > 15 * 60, "admin must not use hardcoded 15m TTL");
    }

    #[tokio::test]
    async fn device_token_carries_its_tenant_and_location() {
        let settings = test_settings("15m");
        let decoding_key = DecodingKey::from_secret(settings.jwt_secret.as_bytes());
        let auth = test_auth_service(settings);
        let device_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let location_id = Uuid::new_v4();

        let token = auth
            .generate_device_token_for(device_id, tenant_id, location_id)
            .expect("device token");
        let mut validation = Validation::default();
        validation.validate_exp = false;
        validation.set_audience(&["gamezone"]);
        validation.set_issuer(&["gamezone"]);
        let claims = decode::<JwtUserClaims>(&token, &decoding_key, &validation)
            .expect("decode")
            .claims;

        assert_eq!(claims.deviceId, Some(device_id.to_string()));
        assert_eq!(claims.tenantId, tenant_id.to_string());
        assert_eq!(claims.locationId, Some(location_id.to_string()));
    }
}

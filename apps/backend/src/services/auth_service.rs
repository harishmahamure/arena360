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
    ActiveSessionDto, AuthResponseDto, CreateSsoTokenDto, CreateSsoTokenResponseDto,
    DevicePairingResponseDto, JwtUserClaims, LoginDto, RateLimitClaims, RedeemSsoTokenDto,
    StaffLoginDto,
};
use crate::error::AppError;
use crate::models::{deduction_profile::DeductionProfile, Device, User};
use crate::repositories::{SessionRepository, SsoRepository, UserRepository};
use crate::services::session_service::display_remaining_for_session;
use crate::services::totp_util::verify_totp_code;
use crate::services::BalanceService;

pub struct AuthService {
    user_repo: UserRepository,
    session_repo: SessionRepository,
    sso_repo: SsoRepository,
    balances: Arc<BalanceService>,
    settings: Arc<Settings>,
}

impl AuthService {
    pub fn new(pool: PgPool, settings: Arc<Settings>, balances: Arc<BalanceService>) -> Self {
        Self {
            user_repo: UserRepository::new(pool.clone()),
            session_repo: SessionRepository::new(pool.clone()),
            sso_repo: SsoRepository::new(pool),
            balances,
            settings,
        }
    }

    pub async fn login_admin(&self, dto: StaffLoginDto) -> Result<AuthResponseDto, AppError> {
        let user = self.authenticate_admin(&dto.username, &dto.password).await?;
        Self::verify_totp_if_enabled(&user, dto.totp.as_deref())?;
        self.issue_auth_response(&user)
    }

    pub async fn login_staff(&self, dto: StaffLoginDto) -> Result<AuthResponseDto, AppError> {
        let user = match self.authenticate_staff(&dto.username, &dto.password).await {
            Ok(u) => u,
            Err(e) => {
                tracing::error!("authenticate_staff failed: {:?}", e);
                return Err(e);
            }
        };

        Self::verify_totp_if_enabled(&user, dto.totp.as_deref())?;

        let token = self.generate_access_token(&user)?;
        Ok(AuthResponseDto {
            accessToken: token,
            user: user.to_auth_user(),
            shiftId: None,
            activeSession: None,
        })
    }

    pub async fn login_player(
        &self,
        device: &Device,
        dto: LoginDto,
    ) -> Result<AuthResponseDto, AppError> {
        if device.registration_status != "registered" {
            return Err(AppError::forbidden_code("DEVICE_NOT_REGISTERED"));
        }

        if device.status == "under_maintenance" {
            return Err(AppError::forbidden_code("DEVICE_UNDER_MAINTENANCE"));
        }

        // Fingerprint drift is enforced in the player-login handler (ADR-0017)
        // before this call, using the optional fingerprint in PlayerLoginDto.

        let user = self
            .authenticate_player(&dto.username, &dto.password)
            .await?;

        let open_session = self
            .session_repo
            .find_open_session_for_player(user.id)
            .await?;

        let active_session = if let Some(session) = open_session {
            if session.device_id != device.id {
                return Err(AppError::conflict_code(
                    "PLAYER_ALREADY_IN_SESSION",
                    Some(json!({
                        "deviceId": session.device_id.to_string(),
                        "deviceName": session.device_name,
                        "sessionId": session.session_id.to_string(),
                        "sessionStartTime": session.start_time.to_rfc3339(),
                    })),
                ));
            }

            let validation = self
                .balances
                .validate_access(session.balance_id, Some(device), None)
                .await?;
            if !validation.valid {
                return Err(BalanceService::validation_to_app_error(validation));
            }

            let balance = self.balances.get_raw(session.balance_id).await?;
            let usage_session = self
                .session_repo
                .find_by_id(session.session_id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound("Open session record missing".to_string())
                })?;
            let remaining = display_remaining_for_session(
                &balance,
                &usage_session,
                &self.settings.cafe_timezone,
            );
            let deduction_profile = balance
                .deduction_profile
                .as_ref()
                .and_then(|value| serde_json::from_value::<DeductionProfile>(value.clone()).ok());
            Some(ActiveSessionDto {
                id: session.session_id.to_string(),
                startTime: session.start_time,
                balanceId: session.balance_id.to_string(),
                remainingMinutes: remaining as f64,
                walletBalanceMinutes: balance.remaining_minutes as f64,
                deductionProfile: deduction_profile,
                cafeTimezone: self.settings.cafe_timezone.clone(),
                timeCreditsConsumed: Some(
                    usage_session.time_credits_consumed.map(|v| v as f64).unwrap_or(0.0),
                ),
                expiryDate: balance.expiry_date,
            })
        } else {
            self.balances
                .require_usable_for_device(user.id, device)
                .await?;
            None
        };

        let token = self.generate_player_token(&user, device.id)?;

        Ok(AuthResponseDto {
            accessToken: token,
            user: user.to_auth_user(),
            shiftId: None,
            activeSession: active_session,
        })
    }

    pub async fn authenticate_player(
        &self,
        username: &str,
        password: &str,
    ) -> Result<User, AppError> {
        let user = self
            .user_repo
            .find_by_username_with_password(username)
            .await?
            .ok_or_else(|| AppError::unauthorized_code("AUTH_INVALID_CREDENTIALS"))?;

        if user.role.as_deref() != Some("player") {
            return Err(AppError::unauthorized_code("AUTH_INVALID_CREDENTIALS"));
        }

        self.verify_password_for_player(password, user.password_hash.as_deref())?;
        self.ensure_active_for_player(&user)?;

        Ok(user)
    }

    pub async fn create_sso_token(
        &self,
        dto: CreateSsoTokenDto,
        created_by: Uuid,
    ) -> Result<CreateSsoTokenResponseDto, AppError> {
        let purpose = dto.purpose.trim();
        if purpose != "tv_provision" && purpose != "tv_login" {
            return Err(AppError::BadRequest(
                "purpose must be tv_provision or tv_login".to_string(),
            ));
        }

        let device_id = match purpose {
            "tv_provision" => {
                let raw = dto
                    .deviceId
                    .as_deref()
                    .ok_or_else(|| AppError::BadRequest("deviceId is required".to_string()))?;
                Some(
                    Uuid::parse_str(raw)
                        .map_err(|_| AppError::BadRequest("Invalid deviceId".to_string()))?,
                )
            }
            _ => dto
                .deviceId
                .as_deref()
                .map(|raw| {
                    Uuid::parse_str(raw)
                        .map_err(|_| AppError::BadRequest("Invalid deviceId".to_string()))
                })
                .transpose()?,
        };

        let raw_token = Uuid::new_v4().to_string();
        let token_hash = hash_sso_token(&raw_token);
        let expires_at = Utc::now() + Duration::minutes(5);

        self.sso_repo
            .insert(&token_hash, purpose, device_id, created_by, expires_at)
            .await?;

        Ok(CreateSsoTokenResponseDto {
            token: raw_token,
            expiresAt: expires_at.to_rfc3339(),
            deviceId: device_id.map(|id| id.to_string()),
        })
    }

    pub async fn redeem_sso_token(
        &self,
        dto: RedeemSsoTokenDto,
    ) -> Result<AuthResponseDto, AppError> {
        let token = dto.token.trim();
        if token.is_empty() {
            return Err(AppError::BadRequest("token is required".to_string()));
        }

        let row = self
            .sso_repo
            .find_valid_by_hash(&hash_sso_token(token))
            .await?
            .ok_or_else(|| AppError::Unauthorized("Invalid or expired SSO token".to_string()))?;

        if row.redeemed_at.is_some() {
            return Err(AppError::Unauthorized("SSO token already used".to_string()));
        }
        if row.expires_at < Utc::now() {
            return Err(AppError::Unauthorized("SSO token expired".to_string()));
        }

        let staff_id = row
            .created_by
            .ok_or_else(|| AppError::Internal("SSO token missing creator".to_string()))?;
        let user = self
            .user_repo
            .find_by_id(staff_id)
            .await?
            .ok_or_else(|| AppError::Unauthorized("SSO creator not found".to_string()))?;

        if !user.role.as_deref().is_some_and(|r| r == "admin" || r == "staff") {
            return Err(AppError::Forbidden("SSO creator is not staff".to_string()));
        }
        self.ensure_active(&user)?;

        self.sso_repo.mark_redeemed(row.id).await?;
        let access_token = self.generate_access_token(&user)?;

        Ok(AuthResponseDto {
            accessToken: access_token,
            user: user.to_auth_user(),
            shiftId: None,
            activeSession: None,
        })
    }

    pub fn generate_pairing_token(&self, device_id: Uuid) -> Result<DevicePairingResponseDto, AppError> {
        let now = Utc::now();
        let expires_at = now + Duration::minutes(5);
        let id = device_id.to_string();

        let claims = JwtUserClaims {
            sub: id.clone(),
            permissions: vec![],
            allowedTenants: vec![],
            rateLimit: Some(RateLimitClaims { qps: 100 }),
            iss: "gamezone".to_string(),
            aud: serde_json::json!("gamezone"),
            iat: Some(now.timestamp()),
            exp: Some(expires_at.timestamp()),
            userId: id.clone(),
            tenantId: "dualshock-arena".to_string(),
            roles: vec!["device_pairing".to_string()],
            appId: "game-zone-console-tv".to_string(),
            orgIds: vec![],
            deviceId: Some(id.clone()),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.settings.jwt_secret.as_bytes()),
        )
        .map_err(AppError::Jwt)?;

        Ok(DevicePairingResponseDto {
            accessToken: token,
            expiresAt: expires_at.to_rfc3339(),
            deviceId: id,
        })
    }

    pub fn generate_device_token(&self, device_id: Uuid) -> Result<String, AppError> {
        let now = Utc::now();
        let exp_duration = parse_duration(&self.settings.jwt_device_expiration);
        let id = device_id.to_string();

        let claims = JwtUserClaims {
            sub: id.clone(),
            permissions: vec![],
            allowedTenants: vec![],
            rateLimit: Some(RateLimitClaims { qps: 100 }),
            iss: "gamezone".to_string(),
            aud: serde_json::json!("gamezone"),
            iat: Some(now.timestamp()),
            exp: Some((now + exp_duration).timestamp()),
            userId: id.clone(),
            tenantId: "dualshock-arena".to_string(),
            roles: vec!["device".to_string()],
            appId: "game-zone-kiosk".to_string(),
            orgIds: vec![],
            deviceId: Some(id),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.settings.jwt_secret.as_bytes()),
        )
        .map_err(AppError::Jwt)
    }

    pub fn generate_player_token(&self, user: &User, device_id: Uuid) -> Result<String, AppError> {
        let now = Utc::now();
        let exp_duration = parse_duration(&self.settings.jwt_player_expiration);

        let claims = JwtUserClaims {
            sub: user.id.to_string(),
            permissions: vec![],
            allowedTenants: vec![],
            rateLimit: Some(RateLimitClaims { qps: 100 }),
            iss: "gamezone".to_string(),
            aud: serde_json::json!("gamezone"),
            iat: Some(now.timestamp()),
            exp: Some((now + exp_duration).timestamp()),
            userId: user.id.to_string(),
            tenantId: "dualshock-arena".to_string(),
            roles: vec!["player".to_string()],
            appId: "game-zone-kiosk".to_string(),
            orgIds: vec![],
            deviceId: Some(device_id.to_string()),
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
            .user_repo
            .find_by_username_with_password(username)
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

    pub fn issue_auth_response(&self, user: &User) -> Result<AuthResponseDto, AppError> {
        let token = self.generate_access_token(user)?;
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
            .user_repo
            .find_by_username_with_password(username)
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
            Some(code) if !code.is_empty() => code,
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

    fn verify_password_for_player(
        &self,
        password: &str,
        hash: Option<&str>,
    ) -> Result<(), AppError> {
        let Some(hash) = hash else {
            return Err(AppError::unauthorized_code("AUTH_INVALID_CREDENTIALS"));
        };
        if verify(password, hash).unwrap_or(false) {
            Ok(())
        } else {
            Err(AppError::unauthorized_code("AUTH_INVALID_CREDENTIALS"))
        }
    }

    fn ensure_active(&self, user: &User) -> Result<(), AppError> {
        if user.is_active {
            Ok(())
        } else {
            Err(AppError::Unauthorized("User is not active".to_string()))
        }
    }

    fn ensure_active_for_player(&self, user: &User) -> Result<(), AppError> {
        if user.is_active {
            Ok(())
        } else {
            Err(AppError::unauthorized_code("AUTH_INVALID_CREDENTIALS"))
        }
    }

    fn generate_access_token(&self, user: &User) -> Result<String, AppError> {
        let role = user.role.clone().unwrap_or_else(|| "player".to_string());
        let now = Utc::now();
        let exp_duration = if role == "admin" {
            Duration::minutes(15)
        } else {
            parse_duration(&self.settings.jwt_access_expiration)
        };

        let claims = JwtUserClaims {
            sub: user.id.to_string(),
            permissions: vec![],
            allowedTenants: vec![],
            rateLimit: Some(RateLimitClaims { qps: 100 }),
            iss: "gamezone".to_string(),
            aud: serde_json::json!("gamezone"),
            iat: Some(now.timestamp()),
            exp: Some((now + exp_duration).timestamp()),
            userId: user.id.to_string(),
            tenantId: "dualshock-arena".to_string(),
            roles: vec![role],
            appId: "game-zone-backend".to_string(),
            orgIds: vec![],
            deviceId: None,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.settings.jwt_secret.as_bytes()),
        )
        .map_err(AppError::Jwt)
    }
}

fn hash_sso_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
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
            Secret::Encoded(secret)
                .to_bytes()
                .expect("secret bytes"),
            Some("GameZone".to_string()),
            "admin".to_string(),
        )
        .expect("totp");
        let code = totp.generate_current().expect("current code");

        AuthService::verify_totp_if_enabled(&user, Some(&code)).expect("valid admin totp");
    }
}

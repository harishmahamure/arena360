use std::sync::Arc;

use bcrypt::verify;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::Settings;
use crate::dto::{
    ActiveSessionDto, AuthResponseDto, JwtUserClaims, LoginDto, OtpPendingResponse, RateLimitClaims,
    StaffLoginDto, VerifyOtpDto,
};
use crate::error::AppError;
use crate::models::{Device, User};
use crate::repositories::{SessionRepository, UserRepository};
use crate::services::totp_util::verify_totp_code;
use crate::services::{BalanceService, MailService, OtpRateLimiter};

pub struct AuthService {
    user_repo: UserRepository,
    session_repo: SessionRepository,
    balances: Arc<BalanceService>,
    mail_service: MailService,
    otp_limiter: OtpRateLimiter,
    settings: Arc<Settings>,
}

impl AuthService {
    pub fn new(pool: PgPool, settings: Arc<Settings>, balances: Arc<BalanceService>) -> Self {
        Self {
            user_repo: UserRepository::new(pool.clone()),
            session_repo: SessionRepository::new(pool),
            balances,
            mail_service: MailService::new(settings.as_ref()),
            otp_limiter: OtpRateLimiter::new(),
            settings,
        }
    }

    pub async fn login_admin(&self, dto: LoginDto) -> Result<OtpPendingResponse, AppError> {
        let user = self
            .user_repo
            .find_by_username_with_password(&dto.username)
            .await?
            .ok_or_else(|| AppError::Unauthorized("User not found".to_string()))?;

        if user.role.as_deref() != Some("admin") {
            return Err(AppError::Unauthorized("User is not an admin".to_string()));
        }

        self.verify_password(&dto.password, user.password_hash.as_deref())?;
        self.ensure_active(&user)?;
        self.otp_limiter.check_and_record(&dto.username)?;

        let session_otp_id = self.send_admin_otp(&user).await?;

        Ok(OtpPendingResponse {
            message: "OTP generated successfully".to_string(),
            transactionId: session_otp_id,
        })
    }

    pub async fn login_staff(&self, dto: StaffLoginDto) -> Result<AuthResponseDto, AppError> {
        let user = match self.authenticate_staff(&dto.username, &dto.password).await {
            Ok(u) => u,
            Err(e) => {
                tracing::error!("authenticate_staff failed: {:?}", e);
                return Err(e);
            }
        };

        if user.totp_enabled {
            let totp_code = match dto.totp {
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

            if !verify_totp_code(secret, &totp_code, &user.username)? {
                tracing::error!("Invalid TOTP code");
                return Err(AppError::Unauthorized("Invalid TOTP code".to_string()));
            }
        }

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

        let user = self.authenticate_player(&dto.username, &dto.password).await?;

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

            Some(ActiveSessionDto {
                id: session.session_id.to_string(),
                startTime: session.start_time,
                balanceId: session.balance_id.to_string(),
                remainingMinutes: session.remaining_minutes as f64,
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

    pub async fn verify_otp(&self, dto: VerifyOtpDto) -> Result<AuthResponseDto, AppError> {
        let user = self
            .user_repo
            .find_by_session_otp_id(&dto.sessionOtpId)
            .await?
            .ok_or_else(|| AppError::Unauthorized("User not found".to_string()))?;

        if user.session_otp.as_deref() != Some(dto.otp.as_str()) {
            return Err(AppError::Unauthorized("Invalid OTP".to_string()));
        }

        let token = self.generate_access_token(&user)?;
        Ok(AuthResponseDto {
            accessToken: token,
            user: user.to_auth_user(),
            shiftId: None,
            activeSession: None,
        })
    }

    async fn send_admin_otp(&self, user: &User) -> Result<String, AppError> {
        let otp_num = (Uuid::new_v4().as_u128() % 900_000) + 100_000;
        let otp = format!("{otp_num:06}");
        let session_otp_id = Uuid::new_v4().to_string();
        self.user_repo
            .update_session_otp(user.id, &session_otp_id, &otp)
            .await?;

        let email = user
            .email
            .clone()
            .unwrap_or_else(|| "hmahamure10@gmail.com".to_string());
        let name = format!(
            "{} {}",
            user.first_name
                .clone()
                .unwrap_or_else(|| "Admin".to_string()),
            user.last_name.clone().unwrap_or_default()
        );

        self.mail_service
            .send_otp_email(&email, name.trim(), &otp)
            .await?;

        Ok(session_otp_id)
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

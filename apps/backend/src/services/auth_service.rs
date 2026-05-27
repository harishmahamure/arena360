use std::sync::Arc;

use bcrypt::verify;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::Settings;
use crate::dto::{
    AuthResponseDto, JwtUserClaims, LoginDto, OtpPendingResponse, RateLimitClaims, VerifyOtpDto,
};
use crate::error::AppError;
use crate::models::User;
use crate::repositories::UserRepository;
use crate::services::totp_util::verify_totp_code;
use crate::services::{MailService, OtpRateLimiter};

pub struct AuthService {
    user_repo: UserRepository,
    mail_service: MailService,
    otp_limiter: OtpRateLimiter,
    settings: Arc<Settings>,
}

impl AuthService {
    pub fn new(pool: PgPool, settings: Arc<Settings>) -> Self {
        Self {
            user_repo: UserRepository::new(pool.clone()),
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

    pub async fn login_staff(&self, dto: LoginDto) -> Result<AuthResponseDto, AppError> {
        let user = self.authenticate_staff(&dto).await?;
        let token = self.generate_access_token(&user)?;
        Ok(AuthResponseDto {
            accessToken: token,
            user: user.to_auth_user(),
            shiftId: None,
        })
    }

    pub async fn authenticate_staff(&self, dto: &LoginDto) -> Result<User, AppError> {
        let user = self
            .user_repo
            .find_by_username_with_password(&dto.username)
            .await?
            .ok_or_else(|| AppError::Unauthorized("User not found".to_string()))?;

        if user.role.as_deref() != Some("staff") {
            return Err(AppError::Unauthorized("User is not staff".to_string()));
        }

        self.verify_password(&dto.password, user.password_hash.as_deref())?;
        self.ensure_active(&user)?;
        Ok(user)
    }

    pub async fn authenticate_staff_with_totp(
        &self,
        username: &str,
        password: &str,
        totp: &str,
    ) -> Result<User, AppError> {
        let user = self
            .authenticate_staff(&LoginDto {
                username: username.to_string(),
                password: password.to_string(),
            })
            .await?;

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

    fn ensure_active(&self, user: &User) -> Result<(), AppError> {
        if user.is_active {
            Ok(())
        } else {
            Err(AppError::Unauthorized("User is not active".to_string()))
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

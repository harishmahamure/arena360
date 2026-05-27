use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterDto {
    pub username: String,
    pub email: Option<String>,
    pub password: String,
    pub firstName: Option<String>,
    pub lastName: Option<String>,
    pub role: Option<String>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, ToSchema)]
pub struct RegisterResponseDto {
    pub message: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginDto {
    pub username: String,
    pub password: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, ToSchema)]
pub struct VerifyOtpDto {
    pub otp: String,
    pub sessionOtpId: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, ToSchema)]
pub struct OtpPendingResponse {
    pub message: String,
    pub transactionId: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponseDto {
    pub accessToken: String,
    pub user: AuthUserDto,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthUserDto {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firstName: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lastName: Option<String>,
    pub role: String,
    pub isActive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitClaims {
    pub qps: i32,
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtUserClaims {
    pub sub: String,
    pub permissions: Vec<String>,
    pub allowedTenants: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rateLimit: Option<RateLimitClaims>,
    pub iss: String,
    pub aud: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iat: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp: Option<i64>,
    pub userId: String,
    pub tenantId: String,
    pub roles: Vec<String>,
    pub appId: String,
    pub orgIds: Vec<String>,
}

impl JwtUserClaims {
    pub fn is_admin(&self) -> bool {
        self.roles.iter().any(|r| r == "admin")
    }

    pub fn is_staff(&self) -> bool {
        self.roles.iter().any(|r| r == "staff")
    }

    pub fn is_admin_or_staff(&self) -> bool {
        self.is_admin() || self.is_staff()
    }

    pub fn user_id_uuid(&self) -> Option<uuid::Uuid> {
        uuid::Uuid::parse_str(&self.userId).ok()
    }
}

#[cfg(test)]
mod jwt_tests {
    use super::*;
    use chrono::{Duration, Utc};
    use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};

    fn sample_claims(now: chrono::DateTime<Utc>, iat: i64) -> JwtUserClaims {
        JwtUserClaims {
            sub: "user-id".to_string(),
            permissions: vec![],
            allowedTenants: vec![],
            rateLimit: Some(RateLimitClaims { qps: 100 }),
            iss: "gamezone".to_string(),
            aud: serde_json::json!("gamezone"),
            iat: Some(iat),
            exp: Some((now + Duration::minutes(15)).timestamp()),
            userId: "user-id".to_string(),
            tenantId: "dualshock-arena".to_string(),
            roles: vec!["admin".to_string()],
            appId: "game-zone-backend".to_string(),
            orgIds: vec![],
        }
    }

    #[test]
    fn jwt_roundtrip_uses_seconds_for_iat() {
        let secret = "your-secret-key-must-be-at-least-32-characters-long";
        let now = Utc::now();
        let claims = sample_claims(now, now.timestamp());
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("encode");

        let mut validation = Validation::default();
        validation.validate_exp = true;
        validation.set_audience(&["gamezone"]);
        validation.set_issuer(&["gamezone"]);

        decode::<JwtUserClaims>(
            &token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &validation,
        )
        .expect("decode with second-based iat");
    }
}

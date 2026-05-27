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
}

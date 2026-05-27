use totp_rs::{Algorithm, Secret, TOTP};

use crate::error::AppError;

const ISSUER: &str = "GameZone";

pub fn generate_totp_setup(account_name: &str) -> Result<(String, String), AppError> {
    let secret = Secret::generate_secret();
    let encoded = secret.to_encoded().to_string();

    let totp = build_totp(&encoded, account_name)?;
    let uri = totp.get_url();

    Ok((encoded, uri))
}

pub fn verify_totp_code(secret: &str, code: &str, account_name: &str) -> Result<bool, AppError> {
    let totp = build_totp(secret, account_name)?;
    totp.check_current(code)
        .map_err(|e| AppError::BadRequest(format!("Invalid TOTP code format: {e}")))
}

fn build_totp(secret: &str, account_name: &str) -> Result<TOTP, AppError> {
    let secret = Secret::Encoded(secret.to_string());
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret.to_bytes().map_err(|e| AppError::Internal(e.to_string()))?,
        Some(ISSUER.to_string()),
        account_name.to_string(),
    )
    .map_err(|e| AppError::Internal(format!("Failed to build TOTP: {e}")))
}

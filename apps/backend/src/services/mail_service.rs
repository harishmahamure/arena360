use reqwest::Client;
use serde_json::json;
use tracing::warn;

use crate::config::Settings;
use crate::error::AppError;

pub struct MailService {
    client: Client,
    token: Option<String>,
}

impl MailService {
    pub fn new(settings: &Settings) -> Self {
        Self {
            client: Client::new(),
            token: settings.zeptomail_token.clone(),
        }
    }

    pub async fn send_otp_email(
        &self,
        email: &str,
        name: &str,
        otp: &str,
    ) -> Result<(), AppError> {
        let Some(token) = &self.token else {
            warn!("ZEPTOMAIL_TOKEN not set; skipping OTP email delivery");
            return Ok(());
        };

        let response = self
            .client
            .post("https://api.zeptomail.in/v1.1/email/template")
            .header("Authorization", token)
            .header("Content-Type", "application/json")
            .json(&json!({
                "mail_template_key": "2518b.67e9c6ada0c3ee78.k1.2b559d50-d07b-11ee-8b58-525400b0b0f3.18dca2a63a5",
                "from": {
                    "address": "noreply@designtemplate.io",
                    "name": "noreply"
                },
                "to": [{
                    "email_address": {
                        "address": email,
                        "name": name
                    }
                }],
                "merge_info": {
                    "name": name,
                    "OTP": otp,
                    "team": "Arena360 Team",
                    "product_name": "Arena360"
                }
            }))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to send email: {e}")))?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("ZeptoMail error: {body}")));
        }

        Ok(())
    }
}
